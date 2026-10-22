use crate::autostart;
use crate::bench::{self, BaselineGate, BenchSummary, Candidate, CandidateSource, Verdict};
use crate::config::{self, AppConfig, GameFilter};
use crate::engine;
use crate::generator::{self, GeneratedStrategy, Seed};
use crate::lock_config;
use crate::lock_unpoisoned;
use crate::probe::{self, HostProbe};
use crate::runtime;
use crate::strategy;
use crate::updater;
use crate::AppResult;
use crate::AppState;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;
use tauri::Manager;
use tauri::State;

mod benchmark;
mod settings;
mod strategies;
mod tgws;
mod updates;
pub use benchmark::*;
pub use settings::*;
pub use strategies::*;
pub use tgws::*;
pub use updates::*;

/// Timeout for one host/protocol probe. Four seconds per protocol is enough for a
/// healthy handshake and keeps a fully broken candidate from eating minutes.
const PROBE_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Serialize)]
pub struct StrategyInfo {
    pub name: String,
    pub file_name: String,
    pub active: bool,
    pub args_preview: String,
}

#[derive(Serialize)]
pub struct GeneratedInfo {
    pub name: String,
    pub parent: String,
    pub score: Option<u32>,
    pub active: bool,
    pub args_preview: String,
}

#[derive(Serialize)]
pub struct AppStatus {
    pub app_version: String,
    pub strategies: Vec<StrategyInfo>,
    pub generated: Vec<GeneratedInfo>,
    pub current_strategy: String,
    pub running: bool,
    pub installed_version: Option<String>,
    pub runtime_valid: bool,
}

#[derive(Serialize)]
pub struct TgwsConfigView {
    pub autostart: bool,
    pub host: String,
    pub port: u16,
    pub secret: String,
    pub installed_version: String,
}

#[derive(Serialize)]
pub struct ConfigView {
    pub autostart: bool,
    pub check_updates: bool,
    pub check_app_updates: bool,
    pub flashscore_bypass: bool,
    pub game_filter: String,
    pub runtime_version: String,
    pub runtime_path: String,
    pub theme: String,
    pub bench_budget: usize,
    pub bench_auto_apply: bool,
    pub tgws: TgwsConfigView,
}

/// Result of an on-demand check: either the current strategy (service running)
/// or the baseline "no bypass" measurement (service stopped).
#[derive(Serialize)]
pub struct QuickCheck {
    pub running: bool,
    pub score: u32,
    pub verdict: Verdict,
    pub gate: BaselineGate,
    pub hosts: Vec<HostProbe>,
}

#[derive(Serialize)]
pub struct UpdateInfo {
    pub installed: Option<String>,
    pub latest: String,
    pub update_available: bool,
    pub changelog: String,
    pub download_size: u64,
}

fn game_filter_of(cfg: &AppConfig) -> GameFilter {
    cfg.game_filter
}

fn current_running(_state: &AppState) -> bool {
    engine::Engine::is_running() || engine::Engine::is_any_winws_running()
}

/// Rejects service-changing actions while a benchmark or update owns the runtime.
fn ensure_idle(state: &AppState) -> Result<(), String> {
    if state.update_running.load(Ordering::SeqCst) {
        return Err("Идёт обновление runtime — дождитесь завершения.".into());
    }
    if state.bench_running.load(Ordering::SeqCst) {
        return Err("Дождитесь окончания проверки стратегий.".into());
    }
    Ok(())
}

fn save_runtime_version_if_needed(state: &AppState) {
    let installed = runtime::installed_version(&config::runtime_dir());
    let mut cfg = lock_config(state);
    if let Some(version) = installed {
        if cfg.runtime_version != version {
            cfg.runtime_version = version;
            let _ = config::save_config(&cfg);
        }
    }
}

fn preview_args(args: &[String]) -> String {
    args.iter()
        .filter(|a| {
            !a.starts_with("--hostlist") && !a.starts_with("--ipset") && !a.starts_with("--ip-id")
        })
        .take(6)
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> AppResult<AppStatus> {
    let cfg = lock_config(&state);
    let game_filter = game_filter_of(&cfg);
    let dir = config::runtime_dir();
    let strategies = strategy::discover_strategies(&dir, game_filter);
    let current = cfg.last_strategy.clone();
    drop(cfg);

    save_runtime_version_if_needed(&state);

    let info: Vec<StrategyInfo> = strategies
        .into_iter()
        .map(|s| StrategyInfo {
            active: s.name == current || s.file_name == current,
            file_name: s.file_name,
            args_preview: preview_args(&s.args),
            name: s.name,
        })
        .collect();

    let generated: Vec<GeneratedInfo> = generator::load_generated()
        .into_iter()
        .map(|g| GeneratedInfo {
            active: g.name == current,
            args_preview: preview_args(&g.args),
            score: g.score,
            parent: g.parent,
            name: g.name,
        })
        .collect();

    Ok(AppStatus {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        strategies: info,
        generated,
        current_strategy: current,
        running: current_running(&state),
        installed_version: runtime::installed_version(&config::runtime_dir()),
        runtime_valid: runtime::is_runtime_valid(&config::runtime_dir()),
    })
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppResult<ConfigView> {
    let cfg = lock_config(&state);
    Ok(ConfigView {
        autostart: cfg.autostart,
        check_updates: cfg.check_updates,
        check_app_updates: cfg.check_app_updates,
        flashscore_bypass: cfg.flashscore_bypass,
        game_filter: cfg.game_filter.as_str().to_string(),
        runtime_version: cfg.runtime_version.clone(),
        runtime_path: config::runtime_dir().to_string_lossy().to_string(),
        theme: cfg.theme.clone(),
        bench_budget: cfg.bench_budget,
        bench_auto_apply: cfg.bench_auto_apply,
        tgws: TgwsConfigView {
            autostart: cfg.tgws.autostart,
            host: cfg.tgws.host.clone(),
            port: cfg.tgws.port,
            secret: cfg.tgws.secret.clone(),
            installed_version: cfg.tgws.installed_version.clone(),
        },
    })
}

#[tauri::command]
pub fn get_whats_new(state: State<'_, AppState>) -> AppResult<Option<crate::whats_new::WhatsNew>> {
    let current = env!("CARGO_PKG_VERSION");
    if lock_config(&state).last_run_version == current {
        return Ok(None);
    }

    let Some(info) = crate::whats_new::current() else {
        let mut cfg = lock_config(&state);
        cfg.last_run_version = current.to_string();
        let _ = config::save_config(&cfg);
        return Ok(None);
    };
    Ok(Some(info))
}

#[tauri::command]
pub fn dismiss_whats_new(state: State<'_, AppState>) -> AppResult<()> {
    let mut cfg = lock_config(&state);
    cfg.last_run_version = env!("CARGO_PKG_VERSION").to_string();
    Ok(config::save_config(&cfg)?)
}

/// Called once at startup. If the previous session died in the middle of a
/// benchmark, the service was stopped and must be restored.
pub fn recover_pending(state: &AppState) {
    let Some(marker) = bench::read_marker() else {
        return;
    };
    if !marker.in_progress {
        bench::clear_marker();
        return;
    }
    let _ = engine::Engine::remove();
    if !marker.previous_strategy.is_empty() {
        let _ = start_inner(state, Some(&marker.previous_strategy));
    }
    bench::clear_marker();
}

fn user_list_files() -> Vec<&'static str> {
    vec![
        "list-general-user.txt",
        "list-exclude-user.txt",
        "ipset-exclude-user.txt",
    ]
}

fn preserve_user_lists(runtime_dir: &std::path::Path) -> Vec<(PathBuf, Vec<u8>)> {
    let lists = runtime_dir.join("lists");
    let mut saved = Vec::new();
    for f in user_list_files() {
        let p = lists.join(f);
        if let Ok(data) = std::fs::read(&p) {
            saved.push((p, data));
        }
    }
    saved
}

fn restore_user_lists(runtime_dir: &std::path::Path, saved: Vec<(PathBuf, Vec<u8>)>) {
    let _ = runtime_dir;
    for (p, data) in saved {
        let _ = std::fs::create_dir_all(p.parent().unwrap_or(std::path::Path::new(".")));
        let _ = std::fs::write(&p, data);
    }
}

/// zapret's winws exits at startup if a `--hostlist`/`--ipset-exclude` file it
/// references is missing (that caused a service crash with exit code 1067).
/// Always make sure the user-list templates exist before starting the bypass.
fn ensure_user_lists(runtime_dir: &std::path::Path) {
    let lists = runtime_dir.join("lists");
    let _ = std::fs::create_dir_all(&lists);
    for f in user_list_files() {
        let p = lists.join(f);
        if !p.exists() {
            let _ = std::fs::write(&p, "");
        }
    }
}

/// Copy custom strategy `.bat` files from the old runtime into the new one.
/// A strategy is considered custom if it is not part of the official release
/// (i.e. the new runtime does not contain a `.bat` with the same name), so
/// user-created strategies like `DYF.bat` survive updates.
fn preserve_custom_strategies(old_dir: &std::path::Path, new_dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(old_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("bat") {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        let target = new_dir.join(name);
        if !target.exists() {
            let _ = std::fs::copy(&path, &target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::preserve_custom_strategies;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let uniq = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("zapret-ui-update-test-{name}-{uniq}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn custom_strategy_survives_update() {
        let old = temp_dir("old");
        let new = temp_dir("new");

        // old runtime: official strategy + custom DYF
        fs::write(old.join("general.bat"), "@echo off\nofficial\n").unwrap();
        fs::write(old.join("DYF.bat"), "@echo off\ncustom\n").unwrap();
        // new runtime: only the official strategy (from the downloaded release)
        fs::write(new.join("general.bat"), "@echo off\nofficial-new\n").unwrap();

        preserve_custom_strategies(&old, &new);

        // DYF.bat (custom, missing in new) must be copied
        assert!(
            new.join("DYF.bat").exists(),
            "DYF.bat должен быть скопирован"
        );
        assert_eq!(
            fs::read_to_string(new.join("DYF.bat")).unwrap(),
            "@echo off\ncustom\n"
        );
        // official strategy must NOT be overwritten by the old copy
        assert_eq!(
            fs::read_to_string(new.join("general.bat")).unwrap(),
            "@echo off\nofficial-new\n"
        );

        let _ = fs::remove_dir_all(&old);
        let _ = fs::remove_dir_all(&new);
    }

    #[test]
    fn non_bat_files_are_ignored() {
        let old = temp_dir("old2");
        let new = temp_dir("new2");

        fs::write(old.join("notes.txt"), "keep or not\n").unwrap();
        fs::write(old.join("service.bat"), "service\n").unwrap();
        fs::write(new.join("service.bat"), "new-service\n").unwrap();

        preserve_custom_strategies(&old, &new);

        assert!(
            !new.join("notes.txt").exists(),
            "txt не должен копироваться"
        );
        assert_eq!(
            fs::read_to_string(new.join("service.bat")).unwrap(),
            "new-service\n",
            "service.bat не должен перезаписываться"
        );

        let _ = fs::remove_dir_all(&old);
        let _ = fs::remove_dir_all(&new);
    }
}
