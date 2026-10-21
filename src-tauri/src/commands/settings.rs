use super::*;
use crate::AppResult;

#[tauri::command]
pub fn set_game_filter(state: State<'_, AppState>, mode: String) -> AppResult<()> {
    let gf = match mode.as_str() {
        "all" => GameFilter::All,
        "tcp" => GameFilter::Tcp,
        "udp" => GameFilter::Udp,
        _ => GameFilter::Disabled,
    };
    {
        let mut cfg = lock_config(&state);
        cfg.game_filter = gf;
        config::save_config(&cfg).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_autostart(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_str = exe.to_string_lossy().to_string();

    // Register/remove the scheduled task FIRST, and only persist the config when
    // the system change actually succeeded (otherwise the UI would show autostart
    // enabled while no task exists).
    if enabled {
        autostart::install(&exe_str)?;
    } else {
        autostart::remove()?;
    }

    let mut cfg = lock_config(&state);
    cfg.autostart = enabled;
    Ok(config::save_config(&cfg).map_err(|e| e.to_string())?)
}

#[tauri::command]
pub fn set_check_updates(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    let mut cfg = lock_config(&state);
    cfg.check_updates = enabled;
    Ok(config::save_config(&cfg).map_err(|e| e.to_string())?)
}

#[tauri::command]
pub fn set_runtime_path(state: State<'_, AppState>, path: String) -> AppResult<String> {
    ensure_idle(&state)?;
    if current_running(&state) {
        return Err("Сначала остановите обход, затем измените папку runtime.".into());
    }

    let trimmed = path.trim();
    let resolved = if trimmed.is_empty() {
        config::app_dir().join(config::RUNTIME_DIR_NAME)
    } else {
        std::path::PathBuf::from(trimmed)
    };
    config::prepare_runtime_dir(&resolved)?;

    {
        let mut cfg = lock_config(&state);
        cfg.runtime_path = if trimmed.is_empty() {
            String::new()
        } else {
            resolved.to_string_lossy().to_string()
        };
        config::save_config(&cfg).map_err(|e| e.to_string())?;
    }
    Ok(resolved.to_string_lossy().to_string())
}

#[tauri::command]
pub fn set_theme(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    theme: String,
) -> AppResult<()> {
    let theme = match theme.as_str() {
        "light" => "light",
        "gray" => "gray",
        _ => "dark",
    };
    {
        let mut cfg = lock_config(&state);
        cfg.theme = theme.to_string();
        config::save_config(&cfg)?;
    }
    if let Some(window) = tauri::Manager::get_webview_window(&app, "main") {
        crate::apply_titlebar_theme(&window, theme != "light");
    }
    Ok(())
}

#[tauri::command]
pub fn set_flashscore_bypass(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    ensure_idle(&state)?;
    {
        let mut cfg = lock_config(&state);
        cfg.flashscore_bypass = enabled;
        config::save_config(&cfg).map_err(|e| e.to_string())?;
    }
    // Apply immediately so the toggle takes effect without a manual restart.
    if current_running(&state) {
        start_inner(&state, None)?;
    }
    Ok(())
}
