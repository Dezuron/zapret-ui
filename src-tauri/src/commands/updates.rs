use super::*;
use crate::AppResult;

#[tauri::command]
pub fn set_check_app_updates(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    let mut cfg = lock_config(&state);
    cfg.check_app_updates = enabled;
    Ok(config::save_config(&cfg).map_err(|e| e.to_string())?)
}

#[tauri::command]
pub async fn check_app_update() -> AppResult<Option<updater::UpdateInfo>> {
    Ok(tauri::async_runtime::spawn_blocking(updater::check)
        .await
        .map_err(|e| e.to_string())??)
}

#[tauri::command]
pub async fn apply_app_update(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    ensure_idle(&state)?;
    let flag = state.update_running.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return Err("Обновление уже выполняется.".into());
    }
    let _guard = OperationGuard(flag);

    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || updater::apply_latest(&app_handle))
        .await
        .map_err(|e| e.to_string())??;

    // The updater spawned the new exe with --wait-for-pid; it will start after
    // this process exits. Give the command response a moment to leave the IPC.
    std::thread::sleep(Duration::from_millis(300));
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn open_external(url: String) -> AppResult<()> {
    if !url.starts_with("https://") {
        return Err("Разрешены только https-ссылки.".into());
    }
    Ok(crate::sys::open_path(std::path::Path::new(&url))?)
}

#[tauri::command]
pub async fn check_update() -> AppResult<UpdateInfo> {
    let release = tauri::async_runtime::spawn_blocking(runtime::fetch_latest_release)
        .await
        .map_err(|e| e.to_string())??;
    let installed = runtime::installed_version(&config::runtime_dir());

    let asset = runtime::find_zip_asset(&release)
        .map(|a| a.size)
        .unwrap_or(0);

    let update_available = runtime::is_update_available(installed.as_deref(), &release.tag_name);

    Ok(UpdateInfo {
        installed,
        latest: release.tag_name.clone(),
        update_available,
        changelog: release.body,
        download_size: asset,
    })
}

#[tauri::command]
pub async fn apply_update(state: State<'_, AppState>, app: tauri::AppHandle) -> AppResult<()> {
    ensure_idle(&state)?;
    let flag = state.update_running.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return Err("Обновление уже выполняется.".into());
    }
    let _guard = OperationGuard(flag);
    let was_running = current_running(&state);
    let old_strategy = {
        let cfg = lock_config(&state);
        cfg.last_strategy.clone()
    };

    let app_handle = app.clone();
    let old_strategy_clone = old_strategy.clone();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        apply_update_blocking(&app_handle, was_running, &old_strategy_clone)
    })
    .await
    .map_err(|e| e.to_string())?;

    match joined {
        Ok(restarted) => {
            // If it was running and not restarted inside, restart it now.
            if was_running && !restarted && !old_strategy.is_empty() {
                start_inner(&state, Some(&old_strategy))?;
            }
            Ok(())
        }
        Err(err) => {
            // The update stops the bypass before swapping files. Restore it only
            // if the update actually stopped it (a network failure happens while
            // the old service is still running and must not restart it).
            if was_running && !old_strategy.is_empty() && !current_running(&state) {
                let restore = start_inner(&state, Some(&old_strategy));
                return Err(with_restore_error(err, &old_strategy, restore).into());
            }
            Err(err.into())
        }
    }
}

/// Returns true if the strategy was restarted inside the blocking call.
fn apply_update_blocking(
    app: &tauri::AppHandle,
    was_running: bool,
    old_strategy: &str,
) -> Result<bool, String> {
    let release = runtime::fetch_latest_release()?;
    let Some(asset) = runtime::find_zip_asset(&release) else {
        return Err("В последнем релизе нет zip-архива с runtime.".into());
    };

    let base = runtime::runtime_tmp_dir();
    runtime::default_cleanup_old_tmp(&base);
    let tmp = runtime::next_tmp_dir(&base);
    let url = asset.browser_download_url.clone();
    let digest = asset.digest.clone();

    let _ = app.emit_to("main", "update-progress", "Скачивание релиза…");

    let zip_path = runtime::download_to_temp(&url, &tmp)?;

    if let Some(digest) = digest.as_deref() {
        if let Err(err) = runtime::verify_digest(&zip_path, digest) {
            let _ = std::fs::remove_file(&zip_path);
            return Err(err);
        }
    }

    let _ = app.emit_to("main", "update-progress", "Распаковка…");
    runtime::extract_zip(&zip_path, &tmp)?;
    std::fs::remove_file(&zip_path).map_err(|e| e.to_string())?;

    if !runtime::is_runtime_valid(&tmp) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err("Скачанный архив не содержит корректный runtime.".into());
    }

    let runtime_dir = config::runtime_dir();

    // Preserve user lists
    let user_lists = preserve_user_lists(&runtime_dir);

    // Stop the bypass (service + winws + driver) before replacing files
    engine::Engine::remove()?;
    std::thread::sleep(std::time::Duration::from_millis(500));

    let _ = app.emit_to("main", "update-progress", "Замена runtime…");

    // Swap: old -> .bak, new -> runtime
    let bak = runtime_dir.with_file_name("runtime.bak");
    if bak.exists() {
        let _ = std::fs::remove_dir_all(&bak);
    }
    if runtime_dir.exists() {
        std::fs::rename(&runtime_dir, &bak).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, &runtime_dir).map_err(|e| {
        // rollback
        if !runtime_dir.exists() && bak.exists() {
            let _ = std::fs::rename(&bak, &runtime_dir);
        }
        e.to_string()
    })?;

    // Restore user lists, and ensure the templates exist (winws exits if a
    // referenced list is missing).
    restore_user_lists(&runtime_dir, user_lists);
    ensure_user_lists(&runtime_dir);

    // Preserve custom strategy .bat files (user-created, e.g. DYF.bat) across
    // the update: copy any *.bat from the backup that is missing in the new
    // runtime. Official strategies are replaced by the new release instead.
    preserve_custom_strategies(&bak, &runtime_dir);

    // Cleanup backup
    let _ = std::fs::remove_dir_all(&bak);

    // Update installed version
    let mut cfg = config::load_config();
    if let Some(v) = runtime::installed_version(&runtime_dir) {
        cfg.runtime_version = v;
    }
    config::save_config(&cfg).map_err(|e| e.to_string())?;

    // Sync in-memory state with the new config
    {
        let state = app.state::<AppState>();
        let mut mem = lock_config(&state);
        *mem = cfg.clone();
    }

    let _ = app.emit_to("main", "update-progress", "Готово");

    // Restart strategy if it was running
    if was_running && !old_strategy.is_empty() {
        let state = app.state::<AppState>();
        start_inner(&state, Some(old_strategy))?;
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
pub fn open_runtime_folder() -> AppResult<()> {
    let dir = config::runtime_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(crate::sys::open_path(&dir)?)
}

// ---------- strategy checks ----------
