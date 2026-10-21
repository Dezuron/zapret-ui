//! Manual end-to-end test of the benchmarking machinery.
//!
//! It stops the `zapret` service, measures real strategies as transient
//! `winws.exe` processes (2 by default, or `ZAPRET_BENCH_LIMIT` many), and always
//! restores the previously active strategy on the way out (including on panic).
//! Run manually, with administrator rights:
//!
//! `cargo test live_bench -- --ignored --nocapture`
//! `$env:ZAPRET_BENCH_LIMIT=22; cargo test live_bench -- --ignored --nocapture`
//!
//! Internet is unstable for the duration of the test.

#[cfg(test)]
mod tests {
    use crate::bench::{self, Candidate, CandidateSource, WinwsHarness};
    use crate::config::{self, GameFilter};
    use crate::engine;
    use crate::probe;
    use crate::strategy;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    struct ServiceRestore {
        dir: PathBuf,
        previous: String,
        game_filter: GameFilter,
        flashscore: bool,
    }

    impl Drop for ServiceRestore {
        fn drop(&mut self) {
            engine::Engine::kill_by_image();
            std::thread::sleep(Duration::from_millis(300));
            let Some(found) = strategy::discover_strategies(&self.dir, self.game_filter)
                .into_iter()
                .find(|s| s.name == self.previous || s.file_name == self.previous)
            else {
                eprintln!("restore: стратегия «{}» не найдена", self.previous);
                return;
            };
            let args = if self.flashscore {
                strategy::with_flashscore_section(&found.args, &self.dir)
            } else {
                found.args
            };
            let winws = self.dir.join("bin").join("winws.exe");
            match engine::Engine::install(&winws.to_string_lossy(), &args, &found.name) {
                Ok(()) => println!(
                    "restore: служба zapret поднята со стратегией «{}»",
                    found.name
                ),
                Err(err) => eprintln!("restore: не удалось поднять службу: {err}"),
            }
        }
    }

    #[test]
    #[ignore]
    fn live_bench() {
        let dir = config::runtime_dir();
        assert!(
            dir.join("bin").join("winws.exe").exists(),
            "runtime отсутствует"
        );
        let cfg = config::load_config();
        assert!(
            !cfg.last_strategy.is_empty(),
            "нет предыдущей стратегии для восстановления"
        );
        assert!(probe::curl_available(), "curl.exe недоступен");

        let targets = probe::load_targets(&dir);
        assert!(!targets.is_empty());

        let limit: usize = std::env::var("ZAPRET_BENCH_LIMIT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(2);
        let candidates: Vec<Candidate> = strategy::discover_strategies(&dir, cfg.game_filter)
            .into_iter()
            .take(limit)
            .map(|s| Candidate {
                name: s.name,
                parent: None,
                args: s.args,
                source: CandidateSource::Official,
            })
            .collect();
        assert!(candidates.len() >= 2, "нужно минимум две стратегии");

        // Restores the service on success, failure and panic.
        let _restore = ServiceRestore {
            dir: dir.clone(),
            previous: cfg.last_strategy.clone(),
            game_filter: cfg.game_filter,
            flashscore: cfg.flashscore_bypass,
        };

        let _ = engine::Engine::remove();
        std::thread::sleep(Duration::from_millis(500));

        let harness = WinwsHarness::new(&dir);
        let outcome = bench::run(
            &candidates,
            &targets,
            Duration::from_secs(4),
            &harness,
            &AtomicBool::new(false),
            |p| println!("[{:>3}%] {}", p.percent, p.message),
        );
        engine::Engine::kill_by_image();

        for result in &outcome.results {
            println!(
                "{}: score={} verdict={:?} D={}/{} Y={}/{} C={}/{} {}мс err={:?}",
                result.name,
                result.score,
                result.verdict,
                result.discord_ok,
                result.discord_total,
                result.youtube_ok,
                result.youtube_total,
                result.control_ok,
                result.control_total,
                result.avg_ms,
                result.error
            );
        }

        assert_eq!(
            outcome.gate,
            bench::BaselineGate::Blocked,
            "без обхода блокировка не обнаружена — проверять нечего"
        );
        assert_eq!(outcome.results.len(), candidates.len());
        assert!(
            outcome.results.iter().all(|r| r.error.is_none()),
            "winws не запустился: {:?}",
            outcome
                .results
                .iter()
                .map(|r| r.error.clone())
                .collect::<Vec<_>>()
        );

        // Persist the real ranking so the desktop UI shows it on the next start.
        let summary = bench::BenchSummary {
            results: outcome.results.clone(),
            baseline: outcome.baseline.clone(),
            gate: outcome.gate,
            cancelled: outcome.cancelled,
            best: bench::best(&outcome.results).map(|r| r.name.clone()),
            applied: None,
            message: Some(
                "Ручной прогон (live_bench): значения реальные, применена прежняя стратегия."
                    .into(),
            ),
            ran_at: bench::now(),
        };
        if let Err(err) = bench::append_history(&summary) {
            eprintln!("не удалось сохранить историю прогона: {err}");
        }
    }
}
