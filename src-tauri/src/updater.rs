//! Application self-update, modelled on Flowseal/tg-ws-proxy:
//! GitHub latest release -> SHA-256 check -> replace the running exe -> restart.
//!
//! Updates are not signed with minisign. The asset must carry a SHA-256 `digest`
//! from the GitHub API; without it the app only offers the release page.

use crate::runtime;
use crate::sys;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

pub const REPO: &str = "Dezuron/zapret-ui";
const ASSET_NAME: &str = "zapret-ui.exe";
const BACKUP_NAME: &str = "zapret-ui_old.exe";
const MAX_UPDATE_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    digest: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub url: String,
    pub notes: String,
    pub size: u64,
    pub can_auto_update: bool,
}

fn client(timeout: Duration) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(format!("zapret-ui/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(timeout)
        .build()
        .map_err(|e| e.to_string())
}

fn fetch_latest() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let response = client(Duration::from_secs(30))?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("Не удалось проверить обновления: {e}"))?;

    // A private repository or a repository without releases returns 404.
    // That is not an error for the user: there is simply nothing to install.
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!("GitHub ответил кодом {}", response.status()));
    }
    response
        .json::<Release>()
        .map(Some)
        .map_err(|e| format!("Некорректный ответ GitHub: {e}"))
}

pub fn normalize_version(tag: &str) -> String {
    tag.trim().trim_start_matches(['v', 'V']).to_string()
}

fn version_parts(version: &str) -> Vec<u64> {
    normalize_version(version)
        .split(['.', '-', '+'])
        .map(|part| {
            part.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let latest = version_parts(latest);
    let current = version_parts(current);
    for index in 0..latest.len().max(current.len()) {
        let left = latest.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

fn valid_digest(digest: Option<&str>) -> bool {
    digest
        .and_then(|value| value.strip_prefix("sha256:"))
        .map(|hex| hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit()))
        .unwrap_or(false)
}

fn select_asset(release: &Release) -> Option<&Asset> {
    release
        .assets
        .iter()
        .find(|asset| asset.name == ASSET_NAME && valid_digest(asset.digest.as_deref()))
}

pub fn check() -> Result<Option<UpdateInfo>, String> {
    let Some(release) = fetch_latest()? else {
        return Ok(None);
    };
    let current = env!("CARGO_PKG_VERSION").to_string();
    let latest = normalize_version(&release.tag_name);
    let size = select_asset(&release).map(|asset| asset.size).unwrap_or(0);
    let can_auto_update = select_asset(&release).is_some();
    Ok(Some(UpdateInfo {
        available: is_newer(&latest, &current),
        latest,
        current,
        url: release.html_url,
        notes: release.body,
        size,
        can_auto_update,
    }))
}

fn download(asset: &Asset) -> Result<PathBuf, String> {
    let digest = asset
        .digest
        .as_deref()
        .filter(|digest| valid_digest(Some(digest)))
        .ok_or("У релиза нет корректной SHA-256 суммы.")?;

    let destination =
        std::env::temp_dir().join(format!("zapret-ui-update-{}.exe", std::process::id()));
    let _ = fs::remove_file(&destination);

    let mut response = client(Duration::from_secs(600))?
        .get(&asset.browser_download_url)
        .send()
        .map_err(|e| format!("Ошибка скачивания обновления: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Ошибка скачивания обновления: {}",
            response.status()
        ));
    }
    if let Some(length) = response.content_length() {
        if length > MAX_UPDATE_BYTES {
            return Err("Файл обновления подозрительно большой.".into());
        }
    }

    let mut file = fs::File::create(&destination).map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        let read = response.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        written += read as u64;
        if written > MAX_UPDATE_BYTES {
            let _ = fs::remove_file(&destination);
            return Err("Файл обновления подозрительно большой.".into());
        }
        file.write_all(&buffer[..read]).map_err(|e| e.to_string())?;
    }

    if let Err(error) = runtime::verify_digest(&destination, digest) {
        let _ = fs::remove_file(&destination);
        return Err(error);
    }
    Ok(destination)
}

fn relaunch_args() -> Vec<OsString> {
    let mut args = Vec::new();
    let mut iterator = std::env::args_os().skip(1);
    while let Some(arg) = iterator.next() {
        if arg == "--wait-for-pid" {
            let _ = iterator.next();
            continue;
        }
        args.push(arg);
    }
    args
}

fn replace_and_restart(downloaded: &Path) -> Result<(), String> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let parent = current
        .parent()
        .ok_or("Не удалось определить папку приложения.")?;
    let backup = parent.join(BACKUP_NAME);

    let _ = fs::remove_file(&backup);
    if backup.exists() {
        return Err("Не удалось удалить старую резервную копию приложения.".into());
    }

    fs::rename(&current, &backup).map_err(|e| format!("Не удалось подготовить замену exe: {e}"))?;

    if let Err(error) = fs::copy(downloaded, &current) {
        let _ = fs::rename(&backup, &current);
        let _ = fs::remove_file(downloaded);
        return Err(format!("Не удалось заменить exe: {error}"));
    }
    let _ = fs::remove_file(downloaded);

    let mut command = Command::new(&current);
    command
        .args(relaunch_args())
        .arg("--wait-for-pid")
        .arg(std::process::id().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    sys::hidden(&mut command);

    if let Err(error) = command.spawn() {
        let _ = fs::remove_file(&current);
        let _ = fs::rename(&backup, &current);
        return Err(format!("Не удалось перезапустить приложение: {error}"));
    }
    Ok(())
}

/// Downloads and installs the latest release. The caller must exit the current
/// process afterwards; the new exe waits for this PID and then starts normally.
pub fn apply_latest(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Emitter;

    let Some(release) = fetch_latest()? else {
        return Err("Релизы приложения недоступны.".into());
    };
    let current = env!("CARGO_PKG_VERSION");
    let latest = normalize_version(&release.tag_name);
    if !is_newer(&latest, current) {
        return Err("Установлена последняя версия приложения.".into());
    }
    let asset = select_asset(&release).ok_or(
        "В релизе нет файла zapret-ui.exe с корректной SHA-256 суммой — обновитесь со страницы релиза.",
    )?;

    let _ = app.emit("app-update-progress", "Скачивание обновления…");
    let downloaded = download(asset)?;
    let _ = app.emit("app-update-progress", "Установка и перезапуск…");
    replace_and_restart(&downloaded)
}

/// Removes the backup left by the previous update after the old process exited.
pub fn cleanup_backup() {
    let Ok(current) = std::env::current_exe() else {
        return;
    };
    let Some(parent) = current.parent() else {
        return;
    };
    let _ = fs::remove_file(parent.join(BACKUP_NAME));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("1.1.1", "1.1.0"));
        assert!(is_newer("v1.2.0", "1.1.9"));
        assert!(!is_newer("1.1.0", "1.1.0"));
        assert!(!is_newer("1.0.9", "1.1.0"));
        assert!(is_newer("2.0", "1.99.99"));
    }

    #[test]
    fn digest_requires_sha256_hex() {
        assert!(valid_digest(Some(
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        )));
        assert!(!valid_digest(Some("md5:deadbeef")));
        assert!(!valid_digest(Some("sha256:short")));
        assert!(!valid_digest(None));
    }

    #[test]
    fn asset_selection_requires_a_valid_digest() {
        let make = |digest: Option<&str>| Release {
            tag_name: "v1.1.1".into(),
            html_url: "https://example".into(),
            body: String::new(),
            assets: vec![Asset {
                name: ASSET_NAME.into(),
                browser_download_url: "https://example/app.exe".into(),
                size: 10,
                digest: digest.map(str::to_string),
            }],
        };
        assert!(select_asset(&make(Some(
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        )))
        .is_some());
        assert!(select_asset(&make(None)).is_none());
        assert!(select_asset(&make(Some("sha256:no"))).is_none());
    }
}
