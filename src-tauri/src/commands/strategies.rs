use super::*;

#[tauri::command]
pub fn set_strategy(state: State<'_, AppState>, name: String) -> AppResult<()> {
    ensure_idle(&state)?;
    let previous = lock_config(&state).last_strategy.clone();
    let was_running = current_running(&state);
    let game_filter = lock_config(&state).game_filter;
    // Resolve before persisting: a typo must not survive as `last_strategy`.
    let (resolved, _args, _) = resolve_strategy(&name, game_filter)?;
    {
        let mut cfg = lock_config(&state);
        cfg.last_strategy = resolved.clone();
        config::save_config(&cfg).map_err(|e| e.to_string())?;
    }
    // If running, restart with the new strategy. A failure must not leave the
    // machine without a bypass: restore the config and the previous service.
    if was_running {
        if let Err(err) = start_inner(&state, Some(&resolved)) {
            {
                let mut cfg = lock_config(&state);
                cfg.last_strategy = previous.clone();
                let _ = config::save_config(&cfg);
            }
            let restore = if previous.is_empty() {
                Ok(())
            } else {
                start_inner(&state, Some(&previous))
            };
            return Err(with_restore_error(err, &previous, restore).into());
        }
    }
    Ok(())
}

/// Resolves a strategy name to its winws arguments. Experimental strategies live
/// in `generated.json`, official ones in the runtime `.bat` files. The bool says
/// whether the strategy is experimental (and therefore needs validation).
fn resolve_strategy(
    name: &str,
    game_filter: GameFilter,
) -> Result<(String, Vec<String>, bool), String> {
    let dir = config::runtime_dir();
    if let Some(found) = strategy::discover_strategies(&dir, game_filter)
        .into_iter()
        .find(|s| s.name == name || s.file_name == name)
    {
        return Ok((found.name, found.args, false));
    }
    if let Some(generated) = generator::find_generated(name) {
        return Ok((generated.name, generated.args, true));
    }
    Err(format!(
        "Стратегия \"{name}\" не найдена. Сначала обновите runtime."
    ))
}

pub(crate) fn start_inner(state: &AppState, name: Option<&str>) -> Result<(), String> {
    let cfg = lock_config(state);
    let strategy_name = name
        .map(|s| s.to_string())
        .unwrap_or_else(|| cfg.last_strategy.clone());
    let game_filter = game_filter_of(&cfg);
    let flashscore_bypass = cfg.flashscore_bypass;
    drop(cfg);

    let dir = config::runtime_dir();
    let (resolved_name, strategy_args, is_generated) =
        resolve_strategy(&strategy_name, game_filter)?;

    let winws = dir.join("bin").join("winws.exe");
    if !winws.exists() {
        return Err("winws.exe не найден в runtime. Сначала обновите runtime.".into());
    }

    // Make sure the user-list files exist (winws exits if a referenced list is missing).
    ensure_user_lists(&dir);

    // generated.json is user-writable; re-validate it at apply time instead of
    // trusting what was written when the candidate was created.
    if is_generated {
        generator::validate_args(&strategy_args).map_err(|e| {
            format!("Экспериментальная стратегия «{resolved_name}» повреждена: {e}")
        })?;
    }

    let args = if flashscore_bypass {
        strategy::with_flashscore_section(&strategy_args, &dir)
    } else {
        strategy_args
    };

    // Install and start winws as the "zapret" Windows service (like service.bat).
    engine::Engine::install(&winws.to_string_lossy(), &args, &resolved_name)?;

    {
        let mut cfg = lock_config(state);
        cfg.last_strategy = resolved_name;
        let _ = config::save_config(&cfg);
    }
    Ok(())
}

#[tauri::command]
pub fn start(state: State<'_, AppState>) -> AppResult<()> {
    ensure_idle(&state)?;
    Ok(start_inner(&state, None)?)
}

#[tauri::command]
pub fn stop(state: State<'_, AppState>) -> AppResult<()> {
    ensure_idle(&state)?;
    // Stop the service and clean up the WinDivert driver (like service.bat "Remove Services").
    engine::Engine::remove()?;
    Ok(())
}
