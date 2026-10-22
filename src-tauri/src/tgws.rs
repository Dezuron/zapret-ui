//! Supervisor for the bundled Telegram MTProto-over-WebSocket proxy.
//!
//! The proxy itself is a headless build of [Flowseal/tg-ws-proxy] (MIT), built
//! in this repository's CI (`tgws-v*` releases) from the upstream console entry
//! point `proxy.tg_ws_proxy:main`. This module owns the process: download,
//! start/stop, port pre-flight, log capture and update checks.
//!
//! [Flowseal/tg-ws-proxy]: https://github.com/Flowseal/tg-ws-proxy

use crate::config;
use crate::lock_config;
use crate::sys;
use crate::AppState;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv6Addr, TcpListener};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

/// Repository that publishes `tgws-v*` releases with the headless binary.
const REPO: &str = "Dezuron/zapret-ui";
const TAG_PREFIX: &str = "tgws-v";
const ASSET_NAME: &str = "tgws-cli-windows-x64.exe";
/// Image name of our own binary; only this image is ever killed by name.
const BINARY_IMAGE: &str = "tgws-cli.exe";

const LOG_LINES: usize = 300;
const LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;
const MIN_DOWNLOAD_BYTES: u64 = 1024 * 1024;
const MAX_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;
const CACHE_TTL_SECS: u64 = 12 * 60 * 60;
/// How long the child gets to fail its `bind()` before we call it healthy.
const START_GRACE: Duration = Duration::from_millis(700);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TgwsState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
    NotInstalled,
}

#[derive(Debug, Clone, Serialize)]
pub struct TgwsStatus {
    pub state: TgwsState,
    pub pid: Option<u32>,
    pub last_error: Option<String>,
    pub installed_version: String,
    pub binary_present: bool,
}

/// Runtime state of the supervisor, stored in [`AppState`].
pub struct Tgws {
    child: Mutex<Option<Child>>,
    status: Mutex<TgwsStatus>,
    logs: Mutex<VecDeque<String>>,
    /// Serialises start/stop/restart so two clicks cannot race.
    op: Mutex<()>,
    cache: Mutex<Option<CachedRelease>>,
    cache_hydrated: Mutex<bool>,
}

impl Default for Tgws {
    fn default() -> Self {
        Self::new()
    }
}

impl Tgws {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
            status: Mutex::new(TgwsStatus {
                state: TgwsState::Stopped,
                pid: None,
                last_error: None,
                installed_version: String::new(),
                binary_present: false,
            }),
            logs: Mutex::new(VecDeque::new()),
            op: Mutex::new(()),
            cache: Mutex::new(None),
            cache_hydrated: Mutex::new(false),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    crate::lock_unpoisoned(mutex)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------- config access ----------

fn load_cfg(app: &AppHandle) -> config::TgwsConfig {
    let state = app.state::<AppState>();
    let cfg = lock_config(&state);
    cfg.tgws.clone()
}

fn save_cfg(app: &AppHandle, patch: impl FnOnce(&mut config::TgwsConfig)) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut cfg = lock_config(&state);
    patch(&mut cfg.tgws);
    config::save_config(&cfg)
}

// ---------- status ----------

pub fn status(app: &AppHandle) -> TgwsStatus {
    let state = app.state::<AppState>();
    let mut snapshot = lock(&state.tgws.status).clone();
    snapshot.binary_present = config::tgws_binary_path().exists();
    snapshot.installed_version = load_cfg(app).installed_version;
    if !snapshot.binary_present
        && !matches!(
            snapshot.state,
            TgwsState::Running | TgwsState::Starting | TgwsState::Stopping
        )
    {
        snapshot.state = TgwsState::NotInstalled;
    }
    snapshot
}

fn set_status(app: &AppHandle, patch: impl FnOnce(&mut TgwsStatus)) -> TgwsStatus {
    let state = app.state::<AppState>();
    let snapshot = {
        let mut status = lock(&state.tgws.status);
        patch(&mut status);
        status.clone()
    };
    let _ = app.emit("tgws-status", &snapshot);
    snapshot
}

fn fail(app: &AppHandle, message: impl Into<String>) -> String {
    let message = message.into();
    set_status(app, |s| {
        s.state = if config::tgws_binary_path().exists() {
            TgwsState::Error
        } else {
            TgwsState::NotInstalled
        };
        s.pid = None;
        s.last_error = Some(message.clone());
    });
    message
}

// ---------- log capture ----------

pub fn mask_secret(line: &str, secret: &str) -> String {
    if secret.is_empty() {
        line.to_string()
    } else {
        line.replace(secret, "***")
    }
}

fn push_log(app: &AppHandle, line: String) {
    let state = app.state::<AppState>();
    {
        let mut logs = lock(&state.tgws.logs);
        if logs.len() >= LOG_LINES {
            logs.pop_front();
        }
        logs.push_back(line.clone());
    }
    append_log_file(&line);
    let _ = app.emit("tgws-log", &line);
}

fn append_log_file(line: &str) {
    let path = config::tgws_log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::metadata(&path)
        .map(|m| m.len() > LOG_MAX_BYTES)
        .unwrap_or(false)
    {
        let _ = fs::rename(&path, path.with_extension("log.1"));
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
    }
}

fn spawn_reader(app: AppHandle, stream: impl Read + Send + 'static, secret: String) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            push_log(&app, mask_secret(&line, &secret));
        }
    });
}

pub fn logs(app: &AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    let logs = lock(&state.tgws.logs);
    logs.iter().cloned().collect()
}

fn log_tail(app: &AppHandle, count: usize) -> String {
    let state = app.state::<AppState>();
    let logs = lock(&state.tgws.logs);
    let start = logs.len().saturating_sub(count);
    logs.iter()
        .skip(start)
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------- secret ----------

pub fn is_valid_secret(secret: &str) -> bool {
    secret.len() == 32 && secret.chars().all(|c| c.is_ascii_hexdigit())
}

fn generate_secret() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("Не удалось сгенерировать ключ: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Returns the persisted secret, generating and saving one when missing or
/// malformed (a hand-edited config must not brick the proxy).
pub fn ensure_secret(app: &AppHandle) -> Result<String, String> {
    let current = load_cfg(app).secret;
    if is_valid_secret(&current) {
        return Ok(current);
    }
    let fresh = generate_secret()?;
    save_cfg(app, |cfg| cfg.secret = fresh.clone())?;
    Ok(fresh)
}

// ---------- link ----------

/// `tg://` link for Telegram Desktop. The `dd` prefix marks a fake-TLS MTProto
/// secret — that is what upstream itself generates, and it is what Telegram
/// accepts for this proxy.
pub fn link_for(host: &str, port: u16, secret: &str) -> String {
    let host = match host.trim() {
        "" | "0.0.0.0" | "::" | "[::]" => "127.0.0.1".to_string(),
        other => other.to_string(),
    };
    let host = if host.parse::<Ipv6Addr>().is_ok() {
        format!("[{host}]")
    } else {
        host
    };
    format!("tg://proxy?server={host}&port={port}&secret=dd{secret}")
}

pub fn link(app: &AppHandle) -> Result<String, String> {
    let secret = ensure_secret(app)?;
    let cfg = load_cfg(app);
    Ok(link_for(&cfg.host, cfg.port, &secret))
}

/// Replaces the secret with a fresh one and restarts a running proxy so the
/// new `tg://` link becomes valid immediately.
pub fn regenerate_secret(app: &AppHandle) -> Result<String, String> {
    let fresh = generate_secret()?;
    save_cfg(app, |cfg| cfg.secret = fresh.clone())?;
    if status(app).state == TgwsState::Running {
        restart(app)?;
    }
    Ok(fresh)
}

/// Applies a settings change and restarts the proxy when it is running, so the
/// UI never shows values that the live process does not use.
pub fn update_settings(
    app: &AppHandle,
    patch: impl FnOnce(&mut config::TgwsConfig),
) -> Result<(), String> {
    save_cfg(app, patch)?;
    if status(app).state == TgwsState::Running {
        restart(app)?;
    }
    Ok(())
}

/// Autostart is read once in `setup`; changing it must not restart the proxy.
pub fn set_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    save_cfg(app, |cfg| cfg.autostart = enabled)
}

// ---------- validation ----------

pub fn validate_host(host: &str) -> Result<String, String> {
    let host = host.trim();
    if host.is_empty() {
        return Err("Укажите хост.".into());
    }
    if host.eq_ignore_ascii_case("localhost") {
        return Ok("localhost".into());
    }
    host.parse::<std::net::IpAddr>()
        .map(|ip| ip.to_string())
        .map_err(|_| "Хост должен быть IP-адресом или localhost.".to_string())
}

pub fn validate_port(port: u16) -> Result<u16, String> {
    if (1024..=65535).contains(&port) {
        Ok(port)
    } else {
        Err("Порт должен быть в диапазоне 1024–65535.".into())
    }
}

pub fn validate_secret(secret: &str) -> Result<String, String> {
    let secret = secret.trim();
    if is_valid_secret(secret) {
        Ok(secret.to_ascii_lowercase())
    } else {
        Err("Ключ должен состоять из 32 шестнадцатеричных символов.".into())
    }
}

// ---------- argv ----------

/// Builds the CLI arguments for the headless upstream entry point. Kept pure so
/// the mapping from config to argv is unit-testable without spawning anything.
pub fn build_args(cfg: &config::TgwsConfig, secret: &str) -> Vec<String> {
    let mut args = vec![
        "--host".to_string(),
        cfg.host.clone(),
        "--port".to_string(),
        cfg.port.to_string(),
        "--secret".to_string(),
        secret.to_string(),
    ];

    for dc in &cfg.dc_ip {
        let dc = dc.trim();
        if !dc.is_empty() {
            args.push("--dc-ip".into());
            args.push(dc.to_string());
        }
    }

    args.push("--buf-kb".into());
    args.push(cfg.buf_kb.max(4).to_string());
    args.push("--pool-size".into());
    args.push(cfg.pool_size.to_string());

    if cfg.verbose {
        args.push("-v".into());
    }
    if !cfg.cfproxy {
        args.push("--no-cfproxy".into());
    }
    if !cfg.h2 {
        args.push("--no-h2".into());
    }

    let domain = cfg.cfproxy_user_domain.trim();
    if !domain.is_empty() {
        args.push("--cfproxy-domain".into());
        args.push(domain.to_string());
    }

    let fake_tls = cfg.fake_tls_domain.trim();
    if !fake_tls.is_empty() {
        args.push("--fake-tls-domain".into());
        args.push(fake_tls.to_string());
    }

    args
}

// ---------- process ----------

fn kill_image() {
    let _ = sys::command("taskkill.exe")
        .args(["/F", "/T", "/IM", BINARY_IMAGE])
        .status();
}

fn port_available(host: &str, port: u16) -> bool {
    TcpListener::bind((host, port)).is_ok()
}

fn ensure_port_free(app: &AppHandle, cfg: &config::TgwsConfig) -> Result<(), String> {
    if port_available(&cfg.host, cfg.port) {
        return Ok(());
    }
    // A leftover tgws-cli.exe from a crash or a manual launch is the usual
    // cause; it is our own image, so killing it by name is safe.
    kill_image();
    std::thread::sleep(Duration::from_millis(600));
    if port_available(&cfg.host, cfg.port) {
        return Ok(());
    }
    Err(fail(
        app,
        format!(
            "Порт {} занят другим приложением. Укажите другой порт в параметрах.",
            cfg.port
        ),
    ))
}

pub fn start(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let _guard = lock(&state.tgws.op);
    start_locked(app)
}

pub fn stop(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let _guard = lock(&state.tgws.op);
    stop_locked(app)
}

pub fn restart(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let _guard = lock(&state.tgws.op);
    stop_locked(app)?;
    std::thread::sleep(Duration::from_millis(300));
    start_locked(app)
}

fn start_locked(app: &AppHandle) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        if lock(&state.tgws.child).is_some() {
            return Ok(());
        }
    }

    if !config::tgws_binary_path().exists() {
        return Err(fail(
            app,
            "Прокси ещё не скачан. Включите его, чтобы загрузить бинарник.",
        ));
    }

    let mut cfg = load_cfg(app);
    cfg.secret = ensure_secret(app)?;

    set_status(app, |s| {
        s.state = TgwsState::Starting;
        s.pid = None;
        s.last_error = None;
    });

    ensure_port_free(app, &cfg)?;

    let binary = config::tgws_binary_path();
    let args = build_args(&cfg, &cfg.secret);
    let mut cmd = Command::new(&binary);
    cmd.args(&args)
        .current_dir(config::tgws_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    sys::hidden(&mut cmd);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => return Err(fail(app, format!("Не удалось запустить прокси: {e}"))),
    };
    let pid = child.id();

    if let Some(stdout) = child.stdout.take() {
        spawn_reader(app.clone(), stdout, cfg.secret.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_reader(app.clone(), stderr, cfg.secret.clone());
    }

    {
        let state = app.state::<AppState>();
        *lock(&state.tgws.child) = Some(child);
    }

    std::thread::sleep(START_GRACE);

    let exited = {
        let state = app.state::<AppState>();
        let mut guard = lock(&state.tgws.child);
        match guard.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(code)) => Some(format!("код {code}")),
                _ => None,
            },
            None => Some("процесс не запустился".to_string()),
        }
    };

    if let Some(reason) = exited {
        let state = app.state::<AppState>();
        *lock(&state.tgws.child) = None;
        kill_image();
        let tail = log_tail(app, 3);
        let message = if tail.is_empty() {
            format!("Прокси завершился сразу после запуска ({reason}).")
        } else {
            format!("Прокси завершился сразу после запуска ({reason}). {tail}")
        };
        return Err(fail(app, message));
    }

    set_status(app, |s| {
        s.state = TgwsState::Running;
        s.pid = Some(pid);
        s.last_error = None;
    });
    spawn_watcher(app.clone(), pid);
    Ok(())
}

/// Detects a proxy that dies on its own and turns it into a visible error.
/// `Child` has no exit event, so a light poll is the only portable option.
fn spawn_watcher(app: AppHandle, pid: u32) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(2));

        let exited = {
            let state = app.state::<AppState>();
            let mut guard = lock(&state.tgws.child);
            match guard.as_mut() {
                Some(child) if child.id() == pid => match child.try_wait() {
                    Ok(Some(status)) => {
                        guard.take();
                        Some(status.to_string())
                    }
                    // The user stopped or restarted the proxy.
                    _ => None,
                },
                _ => return,
            }
        };

        let Some(code) = exited else {
            continue;
        };
        let tail = log_tail(&app, 3);
        let message = if tail.is_empty() {
            format!("Прокси завершился неожиданно ({code}).")
        } else {
            format!("Прокси завершился неожиданно ({code}). {tail}")
        };
        fail(&app, message);
        return;
    });
}

fn stop_locked(app: &AppHandle) -> Result<(), String> {
    let taken = {
        let state = app.state::<AppState>();
        let mut guard = lock(&state.tgws.child);
        guard.take()
    };
    let Some(mut child) = taken else {
        set_status(app, |s| {
            s.state = if config::tgws_binary_path().exists() {
                TgwsState::Stopped
            } else {
                TgwsState::NotInstalled
            };
            s.pid = None;
        });
        return Ok(());
    };

    set_status(app, |s| s.state = TgwsState::Stopping);

    let pid = child.id();
    let _ = sys::command("taskkill.exe")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .status();

    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            break;
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    let _ = child.kill();
    let _ = child.wait();

    set_status(app, |s| {
        s.state = TgwsState::Stopped;
        s.pid = None;
        s.last_error = None;
    });
    Ok(())
}

/// Best-effort synchronous stop used from the application exit hook, where no
/// async runtime or lock ordering guarantees are available any more.
pub fn stop_on_exit(app: &AppHandle) {
    let taken = {
        let state = app.state::<AppState>();
        let mut guard = lock(&state.tgws.child);
        guard.take()
    };
    if let Some(mut child) = taken {
        let pid = child.id();
        let _ = sys::command("taskkill.exe")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .status();
        let _ = child.wait();
    }
    // A manual launch or a previous crash can leave an orphan behind.
    kill_image();
}

// ---------- updates ----------

#[derive(Debug, Clone, Serialize)]
pub struct TgwsUpdateInfo {
    pub installed: String,
    pub latest: Option<String>,
    pub has_update: bool,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub asset_size: u64,
    pub digest: Option<String>,
    pub release_url: Option<String>,
    pub published_at: Option<String>,
    pub dismissed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedRelease {
    at: u64,
    latest: String,
    asset_name: Option<String>,
    asset_url: Option<String>,
    asset_size: u64,
    digest: Option<String>,
    release_url: Option<String>,
    published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

/// Compares `1.11.2-1` style versions. Numeric parts compare numerically, a
/// numeric part beats a textual one, and a missing part counts as `0`.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn parts(value: &str) -> Vec<String> {
        value
            .trim_start_matches(['v', 'V'])
            .split(['.', '-', '+'])
            .map(|part| part.to_string())
            .collect()
    }

    let left = parts(a);
    let right = parts(b);
    for index in 0..left.len().max(right.len()) {
        let lv = left.get(index).map(String::as_str).unwrap_or("0");
        let rv = right.get(index).map(String::as_str).unwrap_or("0");
        let ordering = match (lv.parse::<u64>(), rv.parse::<u64>()) {
            (Ok(l), Ok(r)) => l.cmp(&r),
            (Ok(_), Err(_)) => Ordering::Greater,
            (Err(_), Ok(_)) => Ordering::Less,
            (Err(_), Err(_)) => lv.cmp(rv),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

fn pick_release(releases: Vec<GhRelease>) -> Option<CachedRelease> {
    let mut best: Option<(String, GhRelease)> = None;
    for release in releases {
        let Some(version) = release.tag_name.strip_prefix(TAG_PREFIX) else {
            continue;
        };
        let version = version.trim().to_string();
        if version.is_empty() {
            continue;
        }
        let newer = best
            .as_ref()
            .map(|(current, _)| compare_versions(&version, current) == Ordering::Greater)
            .unwrap_or(true);
        if newer {
            best = Some((version, release));
        }
    }

    let (latest, release) = best?;
    let asset = release.assets.iter().find(|a| a.name == ASSET_NAME);
    Some(CachedRelease {
        at: now_secs(),
        latest,
        asset_name: asset.map(|a| a.name.clone()),
        asset_url: asset.map(|a| a.browser_download_url.clone()),
        asset_size: asset.map(|a| a.size).unwrap_or(0),
        digest: asset.and_then(|a| a.digest.clone()),
        release_url: if release.html_url.is_empty() {
            None
        } else {
            Some(release.html_url)
        },
        published_at: release.published_at,
    })
}

fn cache_path() -> PathBuf {
    config::app_dir().join("tgws_update_cache.json")
}

fn hydrate_cache(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut hydrated = lock(&state.tgws.cache_hydrated);
    if *hydrated {
        return;
    }
    *hydrated = true;
    drop(hydrated);

    let Ok(text) = fs::read_to_string(cache_path()) else {
        return;
    };
    if let Ok(cached) = serde_json::from_str::<CachedRelease>(&text) {
        *lock(&state.tgws.cache) = Some(cached);
    }
}

fn store_cache(app: &AppHandle, cached: &CachedRelease) {
    let state = app.state::<AppState>();
    *lock(&state.tgws.cache) = Some(cached.clone());
    if let Ok(text) = serde_json::to_string(cached) {
        let path = cache_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, text);
    }
}

fn fetch_releases() -> Result<Vec<GhRelease>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=30");
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("zapret-ui/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("Не удалось проверить обновления прокси: {e}"))?;

    // No releases at all (fresh repository) is not an error for the user.
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    if !response.status().is_success() {
        return Err(format!("GitHub ответил кодом {}", response.status()));
    }
    response
        .json::<Vec<GhRelease>>()
        .map_err(|e| format!("Некорректный ответ GitHub: {e}"))
}

fn info_from(cached: &CachedRelease, installed: &str, dismissed_tag: &str) -> TgwsUpdateInfo {
    let installed = installed.trim();
    // Without a downloadable asset there is nothing to offer, even when the
    // version is newer: such a release is incomplete.
    let has_update = cached.asset_url.is_some()
        && (installed.is_empty()
            || compare_versions(&cached.latest, installed) == Ordering::Greater);
    TgwsUpdateInfo {
        installed: installed.to_string(),
        latest: Some(cached.latest.clone()),
        has_update,
        asset_name: cached.asset_name.clone(),
        asset_url: cached.asset_url.clone(),
        asset_size: cached.asset_size,
        digest: cached.digest.clone(),
        release_url: cached.release_url.clone(),
        published_at: cached.published_at.clone(),
        dismissed: !dismissed_tag.trim().is_empty() && dismissed_tag == cached.latest,
    }
}

pub fn check_update(app: &AppHandle, force: bool) -> Result<TgwsUpdateInfo, String> {
    hydrate_cache(app);

    let cfg = load_cfg(app);
    let cached = {
        let state = app.state::<AppState>();
        let guard = lock(&state.tgws.cache);
        guard.clone()
    };

    if let Some(cached) = cached {
        let fresh = now_secs().saturating_sub(cached.at) < CACHE_TTL_SECS;
        if fresh && !force {
            return Ok(info_from(
                &cached,
                &cfg.installed_version,
                &cfg.dismissed_update_tag,
            ));
        }
    }

    let releases = fetch_releases()?;
    let Some(fetched) = pick_release(releases) else {
        return Ok(TgwsUpdateInfo {
            installed: cfg.installed_version.clone(),
            latest: None,
            has_update: false,
            asset_name: None,
            asset_url: None,
            asset_size: 0,
            digest: None,
            release_url: None,
            published_at: None,
            dismissed: false,
        });
    };

    store_cache(app, &fetched);
    Ok(info_from(
        &fetched,
        &cfg.installed_version,
        &cfg.dismissed_update_tag,
    ))
}

pub fn dismiss_update(app: &AppHandle, tag: &str) -> Result<(), String> {
    let tag = tag.trim();
    if tag.is_empty() {
        return Ok(());
    }
    save_cfg(app, |cfg| cfg.dismissed_update_tag = tag.to_string())
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err("Некорректная ссылка на бинарник.".into());
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("zapret-ui/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("Не удалось скачать прокси: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Не удалось скачать прокси: {}", response.status()));
    }
    if let Some(length) = response.content_length() {
        if length > MAX_DOWNLOAD_BYTES {
            return Err("Файл прокси подозрительно большой — загрузка отменена.".into());
        }
    }

    let mut bytes = Vec::new();
    response
        .take(MAX_DOWNLOAD_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Не удалось скачать прокси: {e}"))?;
    if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
        return Err("Файл прокси подозрительно большой — загрузка отменена.".into());
    }
    if (bytes.len() as u64) < MIN_DOWNLOAD_BYTES {
        return Err(format!(
            "Загруженный файл слишком маленький ({} байт) — вероятно, это страница ошибки.",
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn write_binary(bytes: &[u8]) -> Result<(), String> {
    let dir = config::tgws_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Не удалось создать папку прокси: {e}"))?;

    let binary = config::tgws_binary_path();
    let backup = dir.join("tgws-cli.exe.old");
    let _ = fs::remove_file(&backup);
    if binary.exists() {
        fs::rename(&binary, &backup).map_err(|e| format!("Не удалось заменить бинарник: {e}"))?;
    }
    if let Err(e) = fs::write(&binary, bytes) {
        // Roll back so a failed write never leaves the user without a proxy.
        let _ = fs::remove_file(&binary);
        if backup.exists() {
            let _ = fs::rename(&backup, &binary);
        }
        return Err(format!("Не удалось записать бинарник: {e}"));
    }
    let _ = fs::remove_file(&backup);
    Ok(())
}

/// Downloads and installs the headless proxy binary. Returns its size.
pub fn install_update(
    app: &AppHandle,
    url: &str,
    version: &str,
    digest: &str,
) -> Result<u64, String> {
    let expected = digest
        .trim()
        .strip_prefix("sha256:")
        .ok_or("Релиз не содержит SHA-256 — обновление отменено.")?
        .to_string();

    let was_running = status(app).state == TgwsState::Running;
    {
        let state = app.state::<AppState>();
        let _guard = lock(&state.tgws.op);
        stop_locked(app)?;
    }
    // Windows keeps a write lock on the running image even after taskkill
    // returns; kill by name and give the OS a moment.
    kill_image();
    std::thread::sleep(Duration::from_millis(300));

    let bytes = download(url)?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if !actual.eq_ignore_ascii_case(expected.trim()) {
        return Err("Контрольная сумма бинарника не совпала — обновление отменено.".into());
    }

    write_binary(&bytes)?;

    save_cfg(app, |cfg| {
        cfg.installed_version = version.trim().to_string();
        cfg.dismissed_update_tag = String::new();
    })?;

    {
        let state = app.state::<AppState>();
        *lock(&state.tgws.cache) = None;
    }
    let _ = fs::remove_file(cache_path());

    set_status(app, |s| {
        s.state = TgwsState::Stopped;
        s.pid = None;
        s.last_error = None;
        s.binary_present = true;
    });

    if was_running {
        let _ = start(app);
    }
    Ok(bytes.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> config::TgwsConfig {
        config::TgwsConfig::default()
    }

    #[test]
    fn args_contain_required_flags() {
        let secret = "00112233445566778899aabbccddeeff";
        let args = build_args(&cfg(), secret);
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--host" && w[1] == "127.0.0.1"));
        assert!(args.windows(2).any(|w| w[0] == "--port" && w[1] == "1443"));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--secret" && w[1] == secret));
        assert_eq!(args.iter().filter(|a| *a == "--dc-ip").count(), 2);
        assert!(args.windows(2).any(|w| w[0] == "--buf-kb" && w[1] == "256"));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--pool-size" && w[1] == "4"));
        assert!(!args.iter().any(|a| a == "--no-cfproxy"));
        assert!(!args.iter().any(|a| a == "-v"));
    }

    #[test]
    fn args_follow_advanced_config() {
        let mut cfg = cfg();
        cfg.cfproxy = false;
        cfg.h2 = false;
        cfg.verbose = true;
        cfg.buf_kb = 1; // clamped to the upstream minimum of 4
        cfg.pool_size = 0;
        cfg.dc_ip = vec![" ".into()];
        cfg.cfproxy_user_domain = " example.com ".into();
        cfg.fake_tls_domain = "tls.example.com".into();

        let args = build_args(&cfg, "aa");
        assert!(args.iter().any(|a| a == "--no-cfproxy"));
        assert!(args.iter().any(|a| a == "--no-h2"));
        assert!(args.iter().any(|a| a == "-v"));
        assert!(args.windows(2).any(|w| w[0] == "--buf-kb" && w[1] == "4"));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--pool-size" && w[1] == "0"));
        assert!(!args.iter().any(|a| a == "--dc-ip"));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--cfproxy-domain" && w[1] == "example.com"));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--fake-tls-domain" && w[1] == "tls.example.com"));
    }

    #[test]
    fn link_uses_fake_tls_secret_and_loopback() {
        let link = link_for("127.0.0.1", 1443, "00112233445566778899aabbccddeeff");
        assert_eq!(
            link,
            "tg://proxy?server=127.0.0.1&port=1443&secret=dd00112233445566778899aabbccddeeff"
        );
        assert!(link_for("0.0.0.0", 1443, "aa").contains("server=127.0.0.1"));
        assert!(link_for("::", 1443, "aa").contains("server=127.0.0.1"));
    }

    #[test]
    fn link_brackets_ipv6_hosts() {
        let link = link_for("::1", 1443, "aa");
        assert!(
            link.starts_with("tg://proxy?server=[::1]&port=1443"),
            "{link}"
        );
    }

    #[test]
    fn secret_validation_rejects_bad_input() {
        assert!(is_valid_secret("00112233445566778899aabbccddeeff"));
        assert!(is_valid_secret("00112233445566778899AABBCCDDEEFF"));
        assert!(!is_valid_secret("00112233445566778899aabbccddeef"));
        assert!(!is_valid_secret("zz112233445566778899aabbccddeeff"));
        assert_eq!(
            validate_secret(" 00112233445566778899AABBCCDDEEFF ").unwrap(),
            "00112233445566778899aabbccddeeff"
        );
        assert!(validate_secret("nope").is_err());
    }

    #[test]
    fn host_and_port_validation() {
        assert_eq!(validate_host("127.0.0.1").unwrap(), "127.0.0.1");
        assert_eq!(validate_host("LocalHost").unwrap(), "localhost");
        assert!(validate_host("example.com").is_err());
        assert!(validate_host(" ").is_err());
        assert!(validate_port(1443).is_ok());
        assert!(validate_port(80).is_err());
        assert!(validate_port(0).is_err());
    }

    #[test]
    fn version_comparison() {
        use Ordering::*;
        assert_eq!(compare_versions("1.11.2", "1.11.2"), Equal);
        assert_eq!(compare_versions("1.11.2-1", "1.11.2"), Greater);
        assert_eq!(compare_versions("1.11.2", "1.11.10"), Less);
        assert_eq!(compare_versions("v1.11.2", "1.11.2"), Equal);
        assert_eq!(compare_versions("1.11.2-2", "1.11.2-10"), Less);
    }

    #[test]
    fn secret_is_masked_in_logs() {
        let secret = "00112233445566778899aabbccddeeff";
        let line = format!("config secret={secret} port=1443");
        let masked = mask_secret(&line, secret);
        assert!(!masked.contains(secret));
        assert!(masked.contains("***"));
        assert_eq!(mask_secret("nothing", ""), "nothing");
    }

    #[test]
    fn picks_the_newest_tgws_release_with_our_asset() {
        let release = |tag: &str, asset: &str, url: &str| GhRelease {
            tag_name: tag.into(),
            html_url: format!("https://github.com/x/y/releases/tag/{tag}"),
            published_at: None,
            assets: vec![GhAsset {
                name: asset.into(),
                browser_download_url: url.into(),
                size: 15_000_000,
                digest: Some("sha256:abc".into()),
            }],
        };

        let releases = vec![
            release("v1.2.0", ASSET_NAME, "https://github.com/app.exe"),
            release(
                "tgws-v1.11.2",
                ASSET_NAME,
                "https://github.com/tgws-old.exe",
            ),
            release(
                "tgws-v1.11.2-1",
                ASSET_NAME,
                "https://github.com/tgws-new.exe",
            ),
        ];
        let picked = pick_release(releases).expect("release");
        assert_eq!(picked.latest, "1.11.2-1");
        assert_eq!(
            picked.asset_url.as_deref(),
            Some("https://github.com/tgws-new.exe")
        );
        assert_eq!(picked.digest.as_deref(), Some("sha256:abc"));
    }

    #[test]
    fn ignores_releases_without_our_asset() {
        let releases = vec![GhRelease {
            tag_name: "tgws-v1.11.2".into(),
            html_url: String::new(),
            published_at: None,
            assets: vec![],
        }];
        let picked = pick_release(releases).expect("release");
        assert!(picked.asset_url.is_none());
        assert!(picked.release_url.is_none());
    }

    #[test]
    fn update_is_offered_when_installed_version_is_unknown() {
        let cached = CachedRelease {
            at: now_secs(),
            latest: "1.11.2-1".into(),
            asset_name: Some(ASSET_NAME.into()),
            asset_url: Some("https://github.com/a.exe".into()),
            asset_size: 15_000_000,
            digest: Some("sha256:abc".into()),
            release_url: None,
            published_at: None,
        };
        assert!(info_from(&cached, "", "").has_update);
        assert!(!info_from(&cached, "1.11.2-1", "").has_update);
        assert!(info_from(&cached, "1.11.2", "").has_update);
        assert!(info_from(&cached, "1.11.2-1", "1.11.2-1").dismissed);
        assert!(!info_from(&cached, "1.11.2-1", "1.11.2").dismissed);
    }
}
