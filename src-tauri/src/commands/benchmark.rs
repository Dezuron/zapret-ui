use super::*;

/// Bench candidates are built from the raw strategy arguments. The Flashscore
/// profiles the app injects at start-up are hostlist-scoped and cannot affect
/// Discord/YouTube, so leaving them out keeps the measured args identical to the
/// parsed `.bat` and avoids injecting them twice into generated candidates.
fn official_candidates(state: &AppState) -> Vec<Candidate> {
    let game_filter = lock_config(state).game_filter;
    let dir = config::runtime_dir();
    strategy::discover_strategies(&dir, game_filter)
        .into_iter()
        .map(|s| Candidate {
            name: s.name,
            parent: None,
            args: s.args,
            source: CandidateSource::Official,
        })
        .collect()
}

fn generated_candidates() -> Vec<Candidate> {
    generator::load_generated()
        .into_iter()
        .map(|g| Candidate {
            name: g.name,
            parent: Some(g.parent),
            args: g.args,
            source: CandidateSource::Generated,
        })
        .collect()
}

pub(crate) struct OperationGuard(pub(crate) Arc<AtomicBool>);

impl Drop for OperationGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn bench_guard(app: &tauri::AppHandle) -> Result<OperationGuard, String> {
    let state = app.state::<AppState>();
    if state.update_running.load(Ordering::SeqCst) {
        return Err("Идёт обновление runtime — дождитесь завершения.".into());
    }
    let running = state.bench_running.clone();
    if running.swap(true, Ordering::SeqCst) {
        return Err("Проверка стратегий уже идёт.".into());
    }
    state.bench_cancel.store(false, Ordering::SeqCst);
    Ok(OperationGuard(running))
}

fn execute_candidates(
    app: &tauri::AppHandle,
    candidates: &[Candidate],
) -> Result<bench::BenchOutcome, String> {
    let dir = config::runtime_dir();
    let winws = dir.join("bin").join("winws.exe");
    if !winws.exists() {
        return Err("winws.exe не найден в runtime. Сначала обновите runtime.".into());
    }
    let targets = probe::load_targets(&dir);
    if targets.is_empty() {
        return Err("Нет целей для проверки (utils\\targets.txt пуст).".into());
    }
    if !probe::curl_available() {
        return Err("curl.exe недоступен в системе — проверить стратегии нельзя.".into());
    }

    let _ = engine::Engine::remove();
    std::thread::sleep(Duration::from_millis(400));

    let harness = bench::WinwsHarness::new(&dir);
    let cancel = app.state::<AppState>().bench_cancel.clone();
    let emit = app.clone();
    let outcome = bench::run(
        candidates,
        &targets,
        PROBE_TIMEOUT,
        &harness,
        &cancel,
        |progress| {
            let _ = emit.emit("bench-progress", progress);
        },
    );
    engine::Engine::kill_by_image();
    Ok(outcome)
}

fn restore_previous(app: &tauri::AppHandle, previous: &str) -> Result<(), String> {
    if previous.is_empty() {
        return Ok(());
    }
    start_inner(&app.state::<AppState>(), Some(previous))
}

/// Restores the previous strategy only when the bypass is actually down:
/// failures that happen before the service is stopped must not restart it.
fn restore_if_stopped(app: &tauri::AppHandle, previous: &str) -> Result<(), String> {
    if current_running(&app.state::<AppState>()) {
        Ok(())
    } else {
        restore_previous(app, previous)
    }
}

/// Keeps the primary failure visible while still telling the user that the
/// bypass could not be restored — otherwise they would only see the first error.
pub(crate) fn with_restore_error(
    error: String,
    previous: &str,
    restore: Result<(), String>,
) -> String {
    match restore {
        Ok(()) => error,
        Err(restore_error) => {
            format!("{error} (не удалось восстановить «{previous}»: {restore_error})")
        }
    }
}

fn finalize_bench(
    app: &tauri::AppHandle,
    mut outcome: bench::BenchOutcome,
    previous: &str,
    extra_message: Option<String>,
    auto_apply: bool,
) -> Result<BenchSummary, String> {
    bench::sort_results(&mut outcome.results);
    let best_result = bench::best(&outcome.results);
    let best = best_result.map(|r| r.name.clone());
    let best_is_working = best_result
        .map(|r| r.verdict == Verdict::Works)
        .unwrap_or(false);
    let best_score = best_result.map(|r| r.score).unwrap_or(0);
    let state = app.state::<AppState>();
    let mut applied = None;
    let mut message = extra_message;

    if let Some(name) = best.clone() {
        if best_is_working && auto_apply {
            match start_inner(&state, Some(&name)) {
                Ok(()) => applied = Some(name),
                Err(err) => {
                    message = Some(match message {
                        Some(existing) => {
                            format!("{existing} Не удалось применить «{name}»: {err}")
                        }
                        None => format!("Не удалось применить «{name}»: {err}"),
                    })
                }
            }
        } else if best_is_working {
            let text = if previous.is_empty() {
                format!(
                    "Лучший результат — «{name}» ({best_score}/100): работает. Примените вручную."
                )
            } else {
                format!(
                    "Лучший результат — «{name}» ({best_score}/100): работает. Оставлена прежняя стратегия; применить можно вручную."
                )
            };
            message = Some(match message {
                Some(existing) => format!("{existing} {text}"),
                None => text,
            });
        } else {
            // A partial strategy is not auto-applied: silently replacing a working
            // bypass with a 60-89 candidate is worse than leaving the user's choice.
            let text = if previous.is_empty() {
                format!(
                    "Лучший результат — «{name}» ({best_score}/100): работает частично. Примените вручную."
                )
            } else {
                format!(
                    "Лучший результат — «{name}» ({best_score}/100): работает частично. Оставлена прежняя стратегия; применить можно вручную."
                )
            };
            message = Some(match message {
                Some(existing) => format!("{existing} {text}"),
                None => text,
            });
        }
    }
    if applied.is_none() && !previous.is_empty() {
        if let Err(err) = start_inner(&state, Some(previous)) {
            let text = format!("Не удалось восстановить «{previous}»: {err}");
            message = Some(match message {
                Some(existing) => format!("{existing} {text}"),
                None => text,
            });
        }
    }

    bench::clear_marker();
    let summary = BenchSummary {
        results: outcome.results,
        baseline: outcome.baseline,
        gate: outcome.gate,
        cancelled: outcome.cancelled,
        best,
        applied: applied.clone(),
        message,
        ran_at: bench::now(),
    };
    let _ = bench::append_history(&summary);
    *lock_unpoisoned(&state.last_bench) = Some(summary.clone());

    if let Some(name) = applied {
        spawn_watchdog(app.clone(), name, previous.to_string());
    }
    Ok(summary)
}

fn run_bench_session(
    app: &tauri::AppHandle,
    candidates: Vec<Candidate>,
    auto_apply: bool,
) -> Result<BenchSummary, String> {
    let previous = lock_config(&app.state::<AppState>()).last_strategy.clone();

    bench::write_marker(&previous)?;
    match execute_candidates(app, &candidates) {
        Ok(outcome) => finalize_bench(app, outcome, &previous, None, auto_apply),
        Err(err) => {
            // Never leave the machine without a bypass after a failed check.
            bench::clear_marker();
            let restore = restore_if_stopped(app, &previous);
            Err(with_restore_error(err, &previous, restore))
        }
    }
}

fn search_session(
    app: &tauri::AppHandle,
    budget: usize,
    auto_apply: bool,
) -> Result<BenchSummary, String> {
    let budget = budget.clamp(20, 100);
    let state = app.state::<AppState>();
    let previous = lock_config(&state).last_strategy.clone();
    // A cached run is only a valid seed source while it is fresh: DPI behaviour
    // changes with the provider and with time, so stale seeds waste candidates.
    let cached = lock_unpoisoned(&state.last_bench).clone().filter(|run| {
        run.gate == BaselineGate::Blocked
            && !run.results.is_empty()
            && !run.cancelled
            && bench::now().saturating_sub(run.ran_at) <= 600
    });

    bench::write_marker(&previous)?;

    // Phase 1: measure the existing strategies unless a fresh run is available.
    let base_summary = match cached {
        Some(run) => run,
        None => {
            let mut candidates = official_candidates(&state);
            candidates.extend(generated_candidates());
            let outcome = match execute_candidates(app, &candidates) {
                Ok(outcome) => outcome,
                Err(err) => {
                    bench::clear_marker();
                    let restore = restore_if_stopped(app, &previous);
                    return Err(with_restore_error(err, &previous, restore));
                }
            };
            if outcome.gate != BaselineGate::Blocked || outcome.cancelled {
                return finalize_bench(app, outcome, &previous, None, auto_apply);
            }
            BenchSummary {
                results: outcome.results.clone(),
                baseline: outcome.baseline,
                gate: outcome.gate,
                cancelled: false,
                best: None,
                applied: None,
                message: None,
                ran_at: bench::now(),
            }
        }
    };

    // Seeds: the three best candidates, mapped back to their raw arguments.
    let pool = {
        let mut all = official_candidates(&state);
        all.extend(generated_candidates());
        all
    };
    let args_by_name: std::collections::HashMap<&str, &[String]> = pool
        .iter()
        .map(|c| (c.name.as_str(), c.args.as_slice()))
        .collect();
    let seeds: Vec<Seed> = {
        let mut ranked: Vec<&bench::CandidateResult> = base_summary
            .results
            .iter()
            .filter(|r| matches!(r.verdict, Verdict::Works | Verdict::Partial))
            .collect();
        ranked.sort_by(|a, b| b.score.cmp(&a.score).then(a.avg_ms.cmp(&b.avg_ms)));
        ranked
            .iter()
            .take(3)
            .filter_map(|result| {
                args_by_name.get(result.name.as_str()).map(|args| Seed {
                    name: result.name.clone(),
                    args: args.to_vec(),
                })
            })
            .collect()
    };

    if seeds.is_empty() {
        let outcome = bench::BenchOutcome {
            results: base_summary.results,
            baseline: base_summary.baseline,
            gate: base_summary.gate,
            cancelled: false,
        };
        return finalize_bench(app, outcome, &previous, None, auto_apply);
    }

    let runtime = config::runtime_dir();
    // Names must not collide with official .bat strategies either: lookup prefers
    // the official file, so an `auto-1` duplicate would apply the wrong strategy.
    let mut reserved = bench::taken_names();
    reserved.extend(pool.iter().map(|c| c.name.clone()));
    // The clock seed is intentional: `generate` is deterministic for a fixed seed,
    // but repeated searches must explore new values while deduping against the
    // strategies already stored.
    let generated = generator::generate(&seeds, budget, &runtime, bench::now(), &reserved);
    if generated.is_empty() {
        let outcome = bench::BenchOutcome {
            results: base_summary.results,
            baseline: base_summary.baseline,
            gate: base_summary.gate,
            cancelled: false,
        };
        return finalize_bench(app, outcome, &previous, None, auto_apply);
    }

    // Phase 2: measure the generated candidates.
    let gen_candidates: Vec<Candidate> = generated
        .iter()
        .map(|g| Candidate {
            name: g.name.clone(),
            parent: Some(g.parent.clone()),
            args: g.args.clone(),
            source: CandidateSource::Generated,
        })
        .collect();
    let outcome = match execute_candidates(app, &gen_candidates) {
        Ok(outcome) => outcome,
        Err(err) => {
            bench::clear_marker();
            let restore = restore_if_stopped(app, &previous);
            return Err(with_restore_error(err, &previous, restore));
        }
    };
    let cancelled = outcome.cancelled;

    // A second baseline that no longer sees blocking means the network changed
    // under us; silently reporting only phase 1 would look like a valid search.
    if outcome.results.is_empty() && !cancelled && outcome.gate != BaselineGate::Blocked {
        bench::clear_marker();
        let restore = restore_if_stopped(app, &previous);
        return Err(with_restore_error(
            "Повторный замер без обхода не обнаружил блокировку — проверка прервана, попробуйте позже."
                .into(),
            &previous,
            restore,
        ));
    }

    // Keep the winners: everything that works, or the best partial attempt when
    // nothing reached the threshold (so the user still sees the result).
    let mut scored: Vec<(&GeneratedStrategy, &bench::CandidateResult)> = generated
        .iter()
        .filter_map(|g| {
            outcome
                .results
                .iter()
                .find(|r| r.name == g.name)
                .map(|r| (g, r))
        })
        .collect();
    scored.sort_by_key(|(_, r)| std::cmp::Reverse(r.score));

    let any_works = scored.iter().any(|(_, r)| r.verdict == Verdict::Works);
    let winners: Vec<GeneratedStrategy> = scored
        .iter()
        .filter(|(_, r)| {
            (any_works && r.verdict == Verdict::Works)
                || (!any_works && r.verdict == Verdict::Partial)
        })
        .map(|(g, r)| {
            let mut winner = (*g).clone();
            winner.score = Some(r.score);
            winner
        })
        .collect();
    let winners = if any_works {
        winners
    } else {
        winners.into_iter().take(1).collect()
    };
    let save_warning = generator::merge_winners(winners)
        .err()
        .map(|err| format!("Не удалось сохранить экспериментальные стратегии: {err}"));

    let mut results = base_summary.results;
    results.extend(outcome.results);
    let outcome = bench::BenchOutcome {
        results,
        baseline: base_summary.baseline,
        gate: base_summary.gate,
        cancelled,
    };
    finalize_bench(app, outcome, &previous, save_warning, auto_apply)
}

#[tauri::command]
pub async fn run_bench(
    app: tauri::AppHandle,
    include_generated: bool,
    auto_apply: bool,
) -> AppResult<BenchSummary> {
    let _guard = bench_guard(&app)?;
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        {
            let mut cfg = lock_config(&state);
            cfg.bench_auto_apply = auto_apply;
            config::save_config(&cfg)
                .map_err(|e| format!("Не удалось сохранить настройку: {e}"))?;
        }
        let mut candidates = official_candidates(&state);
        if include_generated {
            candidates.extend(generated_candidates());
        }
        run_bench_session(&app, candidates, auto_apply)
    })
    .await
    .map_err(|e| e.to_string())??)
}

#[tauri::command]
pub async fn run_search(
    app: tauri::AppHandle,
    budget: usize,
    auto_apply: bool,
) -> AppResult<BenchSummary> {
    let _guard = bench_guard(&app)?;
    let budget = budget.clamp(20, 100);
    {
        let state = app.state::<AppState>();
        let mut cfg = lock_config(&state);
        cfg.bench_budget = budget;
        cfg.bench_auto_apply = auto_apply;
        config::save_config(&cfg).map_err(|e| format!("Не удалось сохранить настройку: {e}"))?;
    }
    Ok(
        tauri::async_runtime::spawn_blocking(move || search_session(&app, budget, auto_apply))
            .await
            .map_err(|e| e.to_string())??,
    )
}

#[tauri::command]
pub fn set_bench_auto_apply(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    let mut cfg = lock_config(&state);
    cfg.bench_auto_apply = enabled;
    Ok(config::save_config(&cfg).map_err(|e| e.to_string())?)
}

#[tauri::command]
pub fn cancel_bench(state: State<'_, AppState>) -> AppResult<()> {
    state.bench_cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn quick_check(app: tauri::AppHandle) -> AppResult<QuickCheck> {
    let state = app.state::<AppState>();
    ensure_idle(&state)?;
    let running = current_running(&state);

    let result = tauri::async_runtime::spawn_blocking(move || -> Result<QuickCheck, String> {
        if !probe::curl_available() {
            return Err("curl.exe недоступен в системе — проверить стратегии нельзя.".into());
        }
        let targets = probe::load_targets(&config::runtime_dir());
        let hosts = probe::probe_all(&targets, PROBE_TIMEOUT);
        let summary = bench::summarize(&hosts);
        Ok(QuickCheck {
            running,
            score: summary.score,
            verdict: bench::verdict_for(summary.score, summary.control_failed),
            gate: bench::baseline_gate(&hosts),
            hosts,
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    result.map_err(Into::into)
}

#[tauri::command]
pub fn apply_candidate(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> AppResult<()> {
    ensure_idle(&state)?;
    let previous = lock_config(&state).last_strategy.clone();
    if let Err(err) = start_inner(&state, Some(&name)) {
        let restore = if previous.is_empty() {
            Ok(())
        } else {
            start_inner(&state, Some(&previous))
        };
        return Err(with_restore_error(err, &previous, restore).into());
    }
    spawn_watchdog(app, name, previous);
    Ok(())
}

#[tauri::command]
pub fn delete_generated(state: State<'_, AppState>, name: String) -> AppResult<()> {
    if lock_config(&state).last_strategy == name {
        return Err("Нельзя удалить активную стратегию.".into());
    }
    Ok(generator::delete_generated(&name)?)
}

#[tauri::command]
pub fn get_last_bench(state: State<'_, AppState>) -> AppResult<Option<BenchSummary>> {
    if let Some(summary) = lock_unpoisoned(&state.last_bench).clone() {
        return Ok(Some(summary));
    }
    Ok(bench::last_history())
}

/// Verify a freshly applied strategy after a minute and roll back if it turned
/// out to be broken. Runs in the background so it also works with the window hidden.
fn spawn_watchdog(app: tauri::AppHandle, applied: String, previous: String) {
    if previous.is_empty() || previous == applied {
        return;
    }
    let auto = lock_config(&app.state::<AppState>()).apply_auto_rollback;
    if !auto {
        return;
    }

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60));
        let state = app.state::<AppState>();
        if state.bench_running.load(Ordering::SeqCst) {
            return;
        }
        if lock_config(&state).last_strategy != applied {
            return; // the user already switched somewhere else
        }

        let targets = probe::load_targets(&config::runtime_dir());
        let running = engine::Engine::is_running() || engine::Engine::is_any_winws_running();
        // If the service is gone, the user stopped the bypass deliberately; do
        // not resurrect it. A crash leaves the installed service behind.
        if !running && !engine::Engine::is_installed() {
            return;
        }
        let hosts = if running {
            probe::probe_all(&targets, PROBE_TIMEOUT)
        } else {
            Vec::new()
        };
        let summary = bench::summarize(&hosts);
        let verdict = if running {
            bench::verdict_for(summary.score, summary.control_failed)
        } else {
            Verdict::Broken
        };

        match verdict {
            Verdict::Broken | Verdict::Disqualified => {
                match start_inner(&state, Some(&previous)) {
                    Ok(()) => {
                        let _ = app.emit(
                            "auto-rollback",
                            format!(
                                "Стратегия «{applied}» не прошла проверку — возвращена «{previous}»."
                            ),
                        );
                    }
                    Err(err) => {
                        // The user must know the rollback did not happen: they are
                        // still on the strategy that just failed.
                        let _ = app.emit(
                            "bench-warning",
                            format!(
                                "«{applied}» не работает, но вернуть «{previous}» не удалось: {err}"
                            ),
                        );
                    }
                }
            }
            Verdict::Partial => {
                let _ = app.emit(
                    "bench-warning",
                    format!("«{applied}» работает частично ({}/100).", summary.score),
                );
            }
            Verdict::Works => {}
        }
    });
}
