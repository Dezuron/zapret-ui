use crate::config;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/Flowseal/zapret-discord-youtube/releases/latest";

/// Hard ceiling for a runtime archive: a release is ~10 MB, so anything close to
/// this is either a mistake or a zip bomb.
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, serde::Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
    /// GitHub publishes `sha256:<hex>` for release assets. When present, the
    /// downloaded archive is verified before it is extracted and executed.
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

/// Get the version string that the installed runtime reports (from service.bat LOCAL_VERSION).
pub fn installed_version(runtime_dir: &Path) -> Option<String> {
    let p = runtime_dir.join("service.bat");
    let content = fs::read_to_string(p).ok()?;
    let line = content.lines().find(|l| l.contains("LOCAL_VERSION"))?;
    let v = line.split('=').nth(1)?.trim().trim_matches('"').to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

fn http_client(timeout: Duration) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("zapret-ui")
        .connect_timeout(Duration::from_secs(15))
        .timeout(timeout)
        .build()
        .map_err(|e| e.to_string())
}

pub fn fetch_latest_release() -> Result<ReleaseInfo, String> {
    let client = http_client(Duration::from_secs(30))?;
    let resp = client
        .get(LATEST_RELEASE_URL)
        .send()
        .map_err(|e| format!("Не удалось подключиться к GitHub: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub ответил кодом {}", resp.status()));
    }
    resp.json::<ReleaseInfo>().map_err(|e| e.to_string())
}

pub fn find_zip_asset(release: &ReleaseInfo) -> Option<&ReleaseAsset> {
    release.assets.iter().find(|a| a.name.ends_with(".zip"))
}

/// True when the latest release tag differs from the installed version, or when
/// no version is installed (first run).
pub fn is_update_available(installed: Option<&str>, latest: &str) -> bool {
    installed.map(|v| v != latest).unwrap_or(true)
}

/// Download the release archive to a temp file with a fixed local name. The
/// remote asset name is untrusted input and must never become a path component.
pub fn download_to_temp(url: &str, tmp_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(tmp_dir).map_err(|e| e.to_string())?;
    let dest = tmp_dir.join("release.zip");
    let client = http_client(Duration::from_secs(600))?;
    let mut resp = client
        .get(url)
        .send()
        .map_err(|e| format!("Ошибка скачивания: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Ошибка скачивания: {}", resp.status()));
    }
    if let Some(length) = resp.content_length() {
        if length > MAX_ARCHIVE_BYTES {
            return Err("Архив релиза подозрительно большой.".into());
        }
    }

    let mut file = fs::File::create(&dest).map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        let read = resp.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        written += read as u64;
        if written > MAX_ARCHIVE_BYTES {
            let _ = fs::remove_file(&dest);
            return Err("Архив релиза подозрительно большой.".into());
        }
        file.write_all(&buffer[..read]).map_err(|e| e.to_string())?;
    }
    Ok(dest)
}

/// Verifies a GitHub asset digest (`sha256:<hex>`). Returns Ok when the release
/// does not publish a digest: the check is a hardening layer, not a hard gate
/// that would break updates if GitHub changes the field.
pub fn verify_digest(path: &Path, digest: &str) -> Result<(), String> {
    let Some(expected) = digest.strip_prefix("sha256:") else {
        return Ok(());
    };
    let data = fs::read(path).map_err(|e| e.to_string())?;
    let actual = format!("{:x}", Sha256::digest(&data));
    if actual.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err("Контрольная сумма релиза не совпала — обновление отменено.".into())
    }
}

/// Extract a zip into `dest`. If the zip has exactly one top-level folder,
/// move its contents up into `dest` (the release archives are wrapped in a version folder).
pub fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        // `enclosed_name` rejects absolute paths and `..` components, which stops
        // a malicious release archive from writing outside the temp directory.
        let Some(relative) = entry.enclosed_name() else {
            return Err(format!(
                "В архиве релиза недопустимый путь: {}",
                entry.name()
            ));
        };
        if entry.is_dir() {
            continue;
        }
        let out_path = dest.join(&relative);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    // If there is exactly one subdirectory, hoist it up.
    if let Ok(dirs) = fs::read_dir(dest) {
        let subdirs: Vec<PathBuf> = dirs
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect();
        if subdirs.len() == 1 {
            let inner = &subdirs[0];
            for e in fs::read_dir(inner).map_err(|e| e.to_string())? {
                let e = e.map_err(|e| e.to_string())?;
                let src = e.path();
                let dst = dest.join(e.file_name());
                let _ = fs::remove_dir_all(&dst);
                let _ = fs::remove_file(&dst);
                fs::rename(src, dst).map_err(|e| e.to_string())?;
            }
            let _ = fs::remove_dir_all(inner);
        }
    }
    Ok(())
}

/// Check if a runtime looks valid (bin/winws.exe exists).
pub fn is_runtime_valid(runtime_dir: &Path) -> bool {
    runtime_dir.join("bin").join("winws.exe").exists()
}

pub fn runtime_tmp_dir() -> PathBuf {
    config::app_dir().join("tmp")
}

pub fn next_tmp_dir(base: &Path) -> PathBuf {
    let name = format!(
        "runtime_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    base.join(name)
}

pub fn default_cleanup_old_tmp(base: &Path) {
    if let Ok(entries) = fs::read_dir(base) {
        for e in entries.flatten() {
            if e.path().is_dir() && e.file_name().to_string_lossy().starts_with("runtime_") {
                let _ = fs::remove_dir_all(e.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let uniq = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("zapret-ui-test-{name}-{uniq}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn installed_version_parses_service_bat() {
        let dir = temp_dir("version");
        fs::write(
            dir.join("service.bat"),
            "@echo off\nset \"LOCAL_VERSION=1.10.1\"\necho menu\n",
        )
        .unwrap();
        assert_eq!(installed_version(&dir).as_deref(), Some("1.10.1"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn installed_version_none_when_missing() {
        let dir = temp_dir("version-missing");
        assert!(installed_version(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_zip_asset_picks_zip() {
        let release = ReleaseInfo {
            tag_name: "1.10.1".into(),
            body: String::new(),
            assets: vec![
                ReleaseAsset {
                    name: "readme.txt".into(),
                    browser_download_url: "https://example/readme.txt".into(),
                    size: 5,
                    digest: None,
                },
                ReleaseAsset {
                    name: "zapret-discord-youtube-1.10.1.zip".into(),
                    browser_download_url: "https://example/app.zip".into(),
                    size: 1500000,
                    digest: None,
                },
            ],
        };
        let asset = find_zip_asset(&release).expect("zip asset");
        assert!(asset.name.ends_with(".zip"));
        assert_eq!(asset.size, 1500000);
    }

    #[test]
    fn find_zip_asset_none_without_zip() {
        let release = ReleaseInfo {
            tag_name: "1.10.1".into(),
            body: String::new(),
            assets: vec![ReleaseAsset {
                name: "app.exe".into(),
                browser_download_url: "https://example/app.exe".into(),
                size: 0,
                digest: None,
            }],
        };
        assert!(find_zip_asset(&release).is_none());
    }

    #[test]
    fn update_availability_compares_versions() {
        assert!(is_update_available(None, "1.11.0"));
        assert!(is_update_available(Some("1.10.1"), "1.11.0"));
        assert!(!is_update_available(Some("1.11.0"), "1.11.0"));
    }

    #[test]
    fn digest_is_verified_when_present() {
        use sha2::{Digest, Sha256};
        use std::io::Write;

        let dir = temp_dir("digest");
        let file = dir.join("release.zip");
        let mut handle = fs::File::create(&file).unwrap();
        handle.write_all(b"payload").unwrap();
        drop(handle);

        let digest = format!("sha256:{:x}", Sha256::digest(b"payload"));
        assert!(verify_digest(&file, &digest).is_ok());
        assert!(verify_digest(&file, "sha256:00").is_err());
        assert!(verify_digest(&file, "md5:whatever").is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn extract_rejects_path_traversal() {
        use std::io::Write;

        let dir = temp_dir("zip-slip");
        let zip_path = dir.join("evil.zip");
        {
            let file = fs::File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("../escaped.txt", options).unwrap();
            writer.write_all(b"pwned").unwrap();
            writer.finish().unwrap();
        }

        let dest = dir.join("out");
        let result = extract_zip(&zip_path, &dest);
        assert!(result.is_err(), "traversal entry must be rejected");
        assert!(
            !dir.join("escaped.txt").exists(),
            "file must not be written outside the destination"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
