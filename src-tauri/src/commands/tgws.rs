//! Tauri commands for the Telegram MTProto proxy page.
//!
//! Process work (spawning, port pre-flight, downloads) is blocking, so every
//! command that can touch the network or a process runs on the blocking pool.

use crate::tgws::{self, TgwsStatus, TgwsUpdateInfo};
use crate::AppResult;
use std::path::Path;

#[tauri::command]
pub fn get_tgws_status(app: tauri::AppHandle) -> AppResult<TgwsStatus> {
    Ok(tgws::status(&app))
}

#[tauri::command]
pub async fn start_tgws(app: tauri::AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || tgws::start(&app))
        .await
        .map_err(|e| e.to_string())??;
    Ok(())
}

#[tauri::command]
pub async fn stop_tgws(app: tauri::AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || tgws::stop(&app))
        .await
        .map_err(|e| e.to_string())??;
    Ok(())
}

#[tauri::command]
pub async fn restart_tgws(app: tauri::AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || tgws::restart(&app))
        .await
        .map_err(|e| e.to_string())??;
    Ok(())
}

#[tauri::command]
pub fn get_tgws_link(app: tauri::AppHandle) -> AppResult<String> {
    Ok(tgws::link(&app)?)
}

#[tauri::command]
pub async fn regenerate_tgws_secret(app: tauri::AppHandle) -> AppResult<String> {
    Ok(
        tauri::async_runtime::spawn_blocking(move || tgws::regenerate_secret(&app))
            .await
            .map_err(|e| e.to_string())??,
    )
}

#[tauri::command]
pub fn get_tgws_logs(app: tauri::AppHandle) -> AppResult<Vec<String>> {
    Ok(tgws::logs(&app))
}

#[tauri::command]
pub async fn check_tgws_update(
    app: tauri::AppHandle,
    force: Option<bool>,
) -> AppResult<TgwsUpdateInfo> {
    Ok(tauri::async_runtime::spawn_blocking(move || {
        tgws::check_update(&app, force.unwrap_or(false))
    })
    .await
    .map_err(|e| e.to_string())??)
}

#[tauri::command]
pub async fn install_tgws_update(app: tauri::AppHandle) -> AppResult<u64> {
    Ok(
        tauri::async_runtime::spawn_blocking(move || tgws::install_latest(&app))
            .await
            .map_err(|e| e.to_string())??,
    )
}

#[tauri::command]
pub async fn use_tgws_fallback_port(app: tauri::AppHandle) -> AppResult<u16> {
    Ok(
        tauri::async_runtime::spawn_blocking(move || tgws::switch_to_free_port(&app))
            .await
            .map_err(|e| e.to_string())??,
    )
}

#[tauri::command]
pub fn dismiss_tgws_update(app: tauri::AppHandle, tag: String) -> AppResult<()> {
    Ok(tgws::dismiss_update(&app, &tag)?)
}

#[tauri::command]
pub fn set_tgws_autostart(app: tauri::AppHandle, enabled: bool) -> AppResult<()> {
    Ok(tgws::set_autostart(&app, enabled)?)
}

#[tauri::command]
pub async fn set_tgws_host(app: tauri::AppHandle, host: String) -> AppResult<String> {
    let host = tgws::validate_host(&host)?;
    let saved = host.clone();
    tauri::async_runtime::spawn_blocking(move || {
        tgws::update_settings(&app, move |cfg| cfg.host = host)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(saved)
}

#[tauri::command]
pub async fn set_tgws_port(app: tauri::AppHandle, port: u16) -> AppResult<u16> {
    let port = tgws::validate_port(port)?;
    tauri::async_runtime::spawn_blocking(move || {
        tgws::update_settings(&app, move |cfg| cfg.port = port)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(port)
}

#[tauri::command]
pub async fn set_tgws_secret(app: tauri::AppHandle, secret: String) -> AppResult<String> {
    let secret = tgws::validate_secret(&secret)?;
    let saved = secret.clone();
    tauri::async_runtime::spawn_blocking(move || {
        tgws::update_settings(&app, move |cfg| cfg.secret = secret)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(saved)
}

#[tauri::command]
pub fn open_tgws_log() -> AppResult<()> {
    let path = crate::config::tgws_log_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if !path.exists() {
        std::fs::write(&path, b"").map_err(|e| e.to_string())?;
    }
    Ok(crate::sys::open_path(&path)?)
}

/// Opens a `tg://proxy` link with the registered Telegram handler.
#[tauri::command]
pub fn open_telegram_link(app: tauri::AppHandle, url: String) -> AppResult<()> {
    if !url.starts_with("tg://proxy?") {
        return Err("Разрешены только ссылки tg://proxy.".into());
    }
    crate::sys::open_path(Path::new(&url))?;
    // The link has been shown to the user; the automatic hint must not pop up
    // a second dialog later in this session.
    let _ = tgws::mark_link_opened(&app);
    Ok(())
}

#[tauri::command]
pub fn copy_to_clipboard(text: String) -> AppResult<()> {
    Ok(crate::sys::copy_to_clipboard(&text)?)
}
