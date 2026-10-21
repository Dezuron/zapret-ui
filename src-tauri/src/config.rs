use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const APP_DIR_NAME: &str = "zapret-ui";
pub const CONFIG_FILE: &str = "config.json";
pub const RUNTIME_DIR_NAME: &str = "runtime";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameFilter {
    Disabled,
    All,
    Tcp,
    Udp,
}

impl GameFilter {
    pub fn tcp_value(&self) -> &'static str {
        match self {
            GameFilter::Disabled => "12",
            GameFilter::All | GameFilter::Tcp => "1024-65535",
            GameFilter::Udp => "12",
        }
    }

    pub fn udp_value(&self) -> &'static str {
        match self {
            GameFilter::Disabled => "12",
            GameFilter::All | GameFilter::Udp => "1024-65535",
            GameFilter::Tcp => "12",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            GameFilter::Disabled => "disabled",
            GameFilter::All => "all",
            GameFilter::Tcp => "tcp",
            GameFilter::Udp => "udp",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub last_strategy: String,
    pub autostart: bool,
    pub check_updates: bool,
    pub flashscore_bypass: bool,
    pub game_filter: GameFilter,
    pub runtime_version: String,
    /// How many mutants `run_search` generates.
    pub bench_budget: usize,
    /// Verify a freshly applied strategy after a minute and roll back on failure.
    pub apply_auto_rollback: bool,
    /// Apply the best working strategy automatically after a run.
    pub bench_auto_apply: bool,
    /// Check for application updates on startup.
    pub check_app_updates: bool,
    /// Custom folder for the zapret runtime. Empty means `%LOCALAPPDATA%\zapret-ui\runtime`.
    pub runtime_path: String,
    pub last_run_version: String,
    /// UI theme: dark, gray or light.
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            last_strategy: String::new(),
            autostart: true,
            check_updates: true,
            flashscore_bypass: true,
            game_filter: GameFilter::Disabled,
            runtime_version: String::new(),
            bench_budget: 40,
            apply_auto_rollback: true,
            bench_auto_apply: false,
            check_app_updates: true,
            runtime_path: String::new(),
            last_run_version: String::new(),
            theme: "dark".into(),
        }
    }
}

pub fn app_dir() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_DIR_NAME)
}

pub fn config_path() -> PathBuf {
    app_dir().join(CONFIG_FILE)
}

pub fn runtime_dir() -> PathBuf {
    resolve_runtime_dir(&app_dir(), &load_config().runtime_path)
}

/// Resolves the configured runtime folder. An empty value keeps the default
/// under `%LOCALAPPDATA%\zapret-ui\runtime`.
pub fn resolve_runtime_dir(base: &std::path::Path, configured: &str) -> PathBuf {
    let configured = configured.trim();
    if configured.is_empty() {
        base.join(RUNTIME_DIR_NAME)
    } else {
        PathBuf::from(configured)
    }
}

/// Creates the folder and validates that it is writable. A non-empty folder is
/// accepted only when it already contains a zapret runtime, so the app never
/// mixes its files into an unrelated directory.
pub fn prepare_runtime_dir(path: &std::path::Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("Не удалось создать папку: {e}"))?;

    let probe = path.join(".zapret-ui-write-test");
    fs::write(&probe, b"").map_err(|e| format!("Папка недоступна для записи: {e}"))?;
    let _ = fs::remove_file(&probe);

    let has_runtime = path.join("bin").join("winws.exe").exists();
    if !has_runtime {
        let non_empty = fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .flatten()
            .next()
            .is_some();
        if non_empty {
            return Err(
                "Папка не пуста и не содержит runtime zapret. Выберите пустую папку.".into(),
            );
        }
    }
    Ok(())
}

fn parse_config(text: &str) -> Option<AppConfig> {
    // A UTF-8 BOM is valid for many editors but serde_json rejects it. Without
    // stripping it, a hand-edited config silently resets every setting.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    serde_json::from_str(text).ok()
}

pub fn load_config() -> AppConfig {
    match fs::read_to_string(config_path()) {
        Ok(text) => parse_config(&text).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(cfg: &AppConfig) -> Result<(), String> {
    let dir = app_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    let path = config_path();
    // Write-then-rename: a crash or a concurrent read can never observe a
    // half-written config (which used to silently reset every setting).
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("zapret-ui-config-{name}-{unique}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_runtime_dir_is_under_app_data() {
        let base = PathBuf::from(r"C:\base");
        assert_eq!(resolve_runtime_dir(&base, ""), base.join(RUNTIME_DIR_NAME));
        assert_eq!(
            resolve_runtime_dir(&base, "  "),
            base.join(RUNTIME_DIR_NAME)
        );
    }

    #[test]
    fn custom_runtime_dir_is_used_as_is() {
        assert_eq!(
            resolve_runtime_dir(&PathBuf::from(r"C:\base"), r"D:\zapret"),
            PathBuf::from(r"D:\zapret")
        );
    }

    #[test]
    fn prepare_accepts_an_empty_folder() {
        let dir = temp_dir("empty");
        assert!(prepare_runtime_dir(&dir).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prepare_accepts_a_folder_with_a_runtime() {
        let dir = temp_dir("runtime");
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::write(dir.join("bin").join("winws.exe"), b"exe").unwrap();
        assert!(prepare_runtime_dir(&dir).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prepare_rejects_an_unrelated_non_empty_folder() {
        let dir = temp_dir("foreign");
        fs::write(dir.join("notes.txt"), b"not a runtime").unwrap();
        assert!(prepare_runtime_dir(&dir).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_parser_accepts_a_utf8_bom() {
        let config = parse_config("\u{feff}{\"bench_budget\":37}").unwrap();
        assert_eq!(config.bench_budget, 37);
        assert!(config.check_app_updates);
    }

    #[test]
    fn clean_install_has_no_whats_new_marker() {
        assert!(AppConfig::default().last_run_version.is_empty());
    }
}
