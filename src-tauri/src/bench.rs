use crate::config;
use crate::probe::{Group, HostProbe, Status, Target};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const HISTORY_LIMIT: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CandidateSource {
    Official,
    Generated,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub parent: Option<String>,
    pub args: Vec<String>,
    pub source: CandidateSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Works,
    Partial,
    Broken,
    /// A control host failed: the candidate breaks unrelated sites.
    Disqualified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BaselineGate {
    /// Without the bypass the targets are reachable, so there is nothing to tune.
    NotBlocked,
    Blocked,
    /// No target could be measured at all (DNS/network is broken).
    Unmeasurable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostScore {
    pub score: u32,
    pub discord_ok: u32,
    pub discord_total: u32,
    pub youtube_ok: u32,
    pub youtube_total: u32,
    pub control_ok: u32,
    pub control_total: u32,
    pub avg_ms: u64,
    pub control_failed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateResult {
    pub name: String,
    pub source: CandidateSource,
    pub parent: Option<String>,
    pub score: u32,
    pub verdict: Verdict,
    pub discord_ok: u32,
    pub discord_total: u32,
    pub youtube_ok: u32,
    pub youtube_total: u32,
    pub control_ok: u32,
    pub control_total: u32,
    pub avg_ms: u64,
    #[serde(default)]
    pub error: Option<String>,
    pub hosts: Vec<HostProbe>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchProgress {
    pub phase: &'static str,
    pub index: usize,
    pub total: usize,
    pub name: String,
    pub percent: u32,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct BenchOutcome {
    pub results: Vec<CandidateResult>,
    pub baseline: Vec<HostProbe>,
    pub gate: BaselineGate,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchSummary {
    pub results: Vec<CandidateResult>,
    pub baseline: Vec<HostProbe>,
    pub gate: BaselineGate,
    pub cancelled: bool,
    pub best: Option<String>,
    pub applied: Option<String>,
    /// User-facing problem (apply/restore/save failed) that did not abort the run.
    #[serde(default)]
    pub message: Option<String>,
    pub ran_at: u64,
}

/// Everything the bench needs from the outside world. The production harness
/// spawns `winws.exe` and shells out to `curl`; tests substitute a fake one.
pub trait Harness {
    fn start(&self, args: &[String]) -> Result<(), String>;
    fn stop(&self);
    fn probe(&self, targets: &[Target], timeout: Duration) -> Vec<HostProbe>;
}

/// Production harness: runs each candidate as a short-lived `winws.exe` process
/// instead of reinstalling the Windows service for every strategy.
pub struct WinwsHarness {
    pub exe: String,
}

impl WinwsHarness {
    pub fn new(runtime_dir: &std::path::Path) -> Self {
        Self {
            exe: runtime_dir
                .join("bin")
                .join("winws.exe")
                .to_string_lossy()
                .to_string(),
        }
    }
}

impl Harness for WinwsHarness {
    fn start(&self, args: &[String]) -> Result<(), String> {
        // Make sure the previous candidate is gone before a new one binds WinDivert.
        crate::engine::Engine::kill_by_image();
        crate::engine::Engine::spawn_transient(&self.exe, args)?;
        if !crate::engine::Engine::wait_winws_ready(Duration::from_secs(3)) {
            return Err("winws.exe не запустился".into());
        }
        // Give the driver a moment to attach its filters before probing.
        std::thread::sleep(Duration::from_millis(500));
        if !crate::engine::Engine::is_any_winws_running() {
            return Err("winws.exe завершился сразу после запуска".into());
        }
        Ok(())
    }

    fn stop(&self) {
        crate::engine::Engine::kill_by_image();
        std::thread::sleep(Duration::from_millis(250));
    }

    fn probe(&self, targets: &[Target], timeout: Duration) -> Vec<HostProbe> {
        crate::probe::probe_all(targets, timeout)
    }
}

fn weight(group: Group) -> u32 {
    match group {
        Group::Discord | Group::Youtube => 2,
        Group::Control => 1,
    }
}

pub fn summarize(hosts: &[HostProbe]) -> HostScore {
    let mut score = HostScore {
        score: 0,
        discord_ok: 0,
        discord_total: 0,
        youtube_ok: 0,
        youtube_total: 0,
        control_ok: 0,
        control_total: 0,
        avg_ms: 0,
        control_failed: false,
    };
    let mut weight_sum = 0u32;
    let mut weight_ok = 0u32;
    let mut ms_sum = 0u64;
    let mut ms_count = 0u64;

    for host in hosts {
        // A host that rejects every protocol we can speak says nothing about DPI.
        // DNS failures are a resolver problem: zapret cannot fix them, so they
        // must not drag a strategy's score down or disqualify it.
        if matches!(host.status, Status::Unsupported | Status::Dns) {
            continue;
        }
        let ok = host.status == Status::Ok;
        match host.group {
            Group::Discord => {
                score.discord_total += 1;
                if ok {
                    score.discord_ok += 1;
                }
            }
            Group::Youtube => {
                score.youtube_total += 1;
                if ok {
                    score.youtube_ok += 1;
                }
            }
            Group::Control => {
                score.control_total += 1;
                if ok {
                    score.control_ok += 1;
                } else {
                    score.control_failed = true;
                }
            }
        }
        weight_sum += weight(host.group);
        if ok {
            weight_ok += weight(host.group);
            ms_sum += host.ms;
            ms_count += 1;
        }
    }

    score.score = (weight_ok * 100).checked_div(weight_sum).unwrap_or(0);
    score.avg_ms = ms_sum.checked_div(ms_count).unwrap_or(0);
    score
}

pub fn verdict_for(score: u32, control_failed: bool) -> Verdict {
    if control_failed {
        Verdict::Disqualified
    } else if score >= 90 {
        Verdict::Works
    } else if score >= 60 {
        Verdict::Partial
    } else {
        Verdict::Broken
    }
}

/// Compares the no-bypass measurement against the 80% threshold: if the targets
/// are already reachable, every strategy would "pass" and the ranking is noise.
pub fn baseline_gate(hosts: &[HostProbe]) -> BaselineGate {
    let mut total = 0u32;
    let mut ok = 0u32;
    for host in hosts {
        if host.group == Group::Control || matches!(host.status, Status::Unsupported | Status::Dns)
        {
            continue;
        }
        total += 1;
        if host.status == Status::Ok {
            ok += 1;
        }
    }
    if total == 0 {
        BaselineGate::Unmeasurable
    } else if ok * 100 >= total * 80 {
        BaselineGate::NotBlocked
    } else {
        BaselineGate::Blocked
    }
}

pub fn best(results: &[CandidateResult]) -> Option<&CandidateResult> {
    results
        .iter()
        .filter(|r| matches!(r.verdict, Verdict::Works | Verdict::Partial))
        .max_by(|a, b| a.score.cmp(&b.score).then_with(|| b.avg_ms.cmp(&a.avg_ms)))
}

pub fn sort_results(results: &mut [CandidateResult]) {
    results.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.avg_ms.cmp(&b.avg_ms))
            .then_with(|| a.name.cmp(&b.name))
    });
}

pub fn run<H, F>(
    candidates: &[Candidate],
    targets: &[Target],
    timeout: Duration,
    harness: &H,
    cancel: &AtomicBool,
    mut progress: F,
) -> BenchOutcome
where
    H: Harness,
    F: FnMut(&BenchProgress),
{
    progress(&BenchProgress {
        phase: "baseline",
        index: 0,
        total: candidates.len(),
        name: String::new(),
        percent: 0,
        message: "Замер без обхода…".into(),
    });
    let baseline = harness.probe(targets, timeout);
    let gate = baseline_gate(&baseline);

    if gate != BaselineGate::Blocked {
        progress(&BenchProgress {
            phase: "gate",
            index: 0,
            total: candidates.len(),
            name: String::new(),
            percent: 100,
            message: match gate {
                BaselineGate::NotBlocked => "DPI не блокирует цели — перебор не нужен.".into(),
                BaselineGate::Unmeasurable => {
                    "Не удалось замерить цели без обхода (DNS или сеть).".into()
                }
                BaselineGate::Blocked => String::new(),
            },
        });
        return BenchOutcome {
            results: Vec::new(),
            baseline,
            gate,
            cancelled: false,
        };
    }

    let mut results = Vec::with_capacity(candidates.len());
    let mut cancelled = false;

    for (index, candidate) in candidates.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        let percent = ((index as f64 / candidates.len().max(1) as f64) * 100.0) as u32;
        progress(&BenchProgress {
            phase: "candidate",
            index: index + 1,
            total: candidates.len(),
            name: candidate.name.clone(),
            percent,
            message: format!("{}/{}: {}", index + 1, candidates.len(), candidate.name),
        });

        let (hosts, error) = match harness.start(&candidate.args) {
            Ok(()) => (harness.probe(targets, timeout), None),
            Err(err) => (Vec::new(), Some(err)),
        };
        harness.stop();

        let host_score = summarize(&hosts);
        results.push(CandidateResult {
            name: candidate.name.clone(),
            source: candidate.source,
            parent: candidate.parent.clone(),
            score: host_score.score,
            verdict: verdict_for(host_score.score, host_score.control_failed),
            discord_ok: host_score.discord_ok,
            discord_total: host_score.discord_total,
            youtube_ok: host_score.youtube_ok,
            youtube_total: host_score.youtube_total,
            control_ok: host_score.control_ok,
            control_total: host_score.control_total,
            avg_ms: host_score.avg_ms,
            error,
            hosts,
        });
    }

    if cancel.load(Ordering::Relaxed) {
        cancelled = true;
    }
    progress(&BenchProgress {
        phase: "done",
        index: results.len(),
        total: candidates.len(),
        name: String::new(),
        percent: 100,
        message: if cancelled {
            "Проверка отменена.".into()
        } else {
            "Проверка завершена.".into()
        },
    });

    BenchOutcome {
        results,
        baseline,
        gate,
        cancelled,
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------- crash marker ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchMarker {
    pub in_progress: bool,
    #[serde(default)]
    pub previous_strategy: String,
}

pub fn marker_path() -> PathBuf {
    config::app_dir().join("bench_state.json")
}

pub fn write_marker(previous_strategy: &str) -> Result<(), String> {
    let path = marker_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let marker = BenchMarker {
        in_progress: true,
        previous_strategy: previous_strategy.to_string(),
    };
    let json = serde_json::to_string_pretty(&marker).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn read_marker() -> Option<BenchMarker> {
    let text = std::fs::read_to_string(marker_path()).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn clear_marker() {
    let _ = std::fs::remove_file(marker_path());
}

// ---------- history ----------

pub fn history_path() -> PathBuf {
    config::app_dir().join("bench.json")
}

pub fn load_history() -> Vec<BenchSummary> {
    let path = history_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<BenchSummary>>(&text) {
        Ok(history) => history,
        Err(_) => {
            // Keep the unreadable file for diagnostics instead of overwriting it.
            let _ = std::fs::rename(&path, path.with_extension("corrupt.json"));
            Vec::new()
        }
    }
}

pub fn append_history(summary: &BenchSummary) -> Result<(), String> {
    let mut history = load_history();
    history.push(summary.clone());
    if history.len() > HISTORY_LIMIT {
        let excess = history.len() - HISTORY_LIMIT;
        history.drain(0..excess);
    }
    let path = history_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string(&history).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn last_history() -> Option<BenchSummary> {
    load_history().last().cloned()
}

/// Names already used by saved experimental strategies.
pub fn taken_names() -> HashSet<String> {
    crate::generator::load_generated()
        .into_iter()
        .map(|s| s.name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock_unpoisoned;
    use std::sync::Mutex;

    fn host(name: &str, group: Group, status: Status) -> HostProbe {
        HostProbe {
            name: name.into(),
            url: format!("https://{name}.test"),
            group,
            status,
            ms: if status == Status::Ok { 120 } else { 0 },
            detail: String::new(),
        }
    }

    fn target_hosts(failing: bool) -> Vec<HostProbe> {
        let status = if failing { Status::Reset } else { Status::Ok };
        let mut hosts = Vec::new();
        for i in 0..4 {
            hosts.push(host(&format!("d{i}"), Group::Discord, status));
            hosts.push(host(&format!("y{i}"), Group::Youtube, status));
        }
        for i in 0..4 {
            hosts.push(host(&format!("c{i}"), Group::Control, Status::Ok));
        }
        hosts
    }

    struct FakeHarness {
        baseline: Vec<HostProbe>,
        probes: Mutex<std::collections::VecDeque<Vec<HostProbe>>>,
        start_errors: Mutex<std::collections::VecDeque<Option<String>>>,
        baseline_done: std::sync::atomic::AtomicBool,
        started: Mutex<Vec<Vec<String>>>,
    }

    impl FakeHarness {
        fn new(baseline: Vec<HostProbe>, probes: Vec<Vec<HostProbe>>) -> Self {
            Self {
                baseline,
                probes: Mutex::new(probes.into_iter().collect()),
                start_errors: Mutex::new(std::collections::VecDeque::new()),
                baseline_done: std::sync::atomic::AtomicBool::new(false),
                started: Mutex::new(Vec::new()),
            }
        }

        fn with_start_errors(mut self, errors: Vec<Option<String>>) -> Self {
            self.start_errors = Mutex::new(errors.into_iter().collect());
            self
        }
    }

    impl Harness for FakeHarness {
        fn start(&self, args: &[String]) -> Result<(), String> {
            lock_unpoisoned(&self.started).push(args.to_vec());
            match lock_unpoisoned(&self.start_errors).pop_front().flatten() {
                Some(err) => Err(err),
                None => Ok(()),
            }
        }

        fn stop(&self) {}

        fn probe(&self, _targets: &[Target], _timeout: Duration) -> Vec<HostProbe> {
            if !self
                .baseline_done
                .swap(true, std::sync::atomic::Ordering::SeqCst)
            {
                return self.baseline.clone();
            }
            lock_unpoisoned(&self.probes)
                .pop_front()
                .unwrap_or_default()
        }
    }

    fn candidate(name: &str) -> Candidate {
        Candidate {
            name: name.into(),
            parent: None,
            args: vec!["--hostlist=x".into()],
            source: CandidateSource::Official,
        }
    }

    fn run_with(
        harness: &FakeHarness,
        candidates: &[Candidate],
        cancel: &AtomicBool,
    ) -> (BenchOutcome, Vec<String>) {
        let mut messages = Vec::new();
        let outcome = run(
            candidates,
            &[],
            Duration::from_millis(1),
            harness,
            cancel,
            |p| messages.push(p.message.clone()),
        );
        (outcome, messages)
    }

    #[test]
    fn scoring_weights_targets_twice_and_needs_all_controls() {
        let all_ok = summarize(&target_hosts(false));
        assert_eq!(all_ok.score, 100);
        assert_eq!(
            verdict_for(all_ok.score, all_ok.control_failed),
            Verdict::Works
        );
        assert_eq!((all_ok.discord_ok, all_ok.discord_total), (4, 4));

        let all_bad = summarize(&target_hosts(true));
        assert_eq!(all_bad.score, 20, "only the control hosts still pass");
        assert_eq!(
            verdict_for(all_bad.score, all_bad.control_failed),
            Verdict::Broken
        );
    }

    #[test]
    fn one_failing_control_host_disqualifies_a_perfect_score() {
        let mut hosts = target_hosts(false);
        hosts[10].status = Status::Timeout; // first control host
        let summary = summarize(&hosts);
        assert_eq!(summary.score, 95);
        assert!(summary.control_failed);
        assert_eq!(
            verdict_for(summary.score, summary.control_failed),
            Verdict::Disqualified
        );
    }

    #[test]
    fn unsupported_hosts_are_neutral() {
        let mut hosts = target_hosts(false);
        hosts.push(host("weird", Group::Control, Status::Unsupported));
        let summary = summarize(&hosts);
        assert_eq!(summary.score, 100);
        assert_eq!(summary.control_total, 4, "unsupported host is not counted");
    }

    #[test]
    fn dns_failures_are_not_scored_against_the_strategy() {
        let mut hosts = target_hosts(false);
        hosts[0].status = Status::Dns; // a Discord host cannot be resolved
        hosts[8].status = Status::Dns; // a control host cannot be resolved
        let summary = summarize(&hosts);
        assert_eq!(
            summary.score, 100,
            "resolver problems must not lower the score"
        );
        assert!(!summary.control_failed, "zapret does not control DNS");
        assert_eq!(summary.discord_total, 3);
        assert_eq!(summary.control_total, 3);
    }

    #[test]
    fn baseline_with_only_dns_failures_is_unmeasurable() {
        let mut hosts = target_hosts(true);
        for host in &mut hosts {
            host.status = Status::Dns;
        }
        assert_eq!(baseline_gate(&hosts), BaselineGate::Unmeasurable);
    }

    #[test]
    fn baseline_gate_skips_when_targets_already_work() {
        assert_eq!(
            baseline_gate(&target_hosts(false)),
            BaselineGate::NotBlocked
        );
        assert_eq!(baseline_gate(&target_hosts(true)), BaselineGate::Blocked);
        assert_eq!(baseline_gate(&[]), BaselineGate::Unmeasurable);
    }

    #[test]
    fn bench_skips_when_dpi_is_not_blocking() {
        let harness = FakeHarness::new(target_hosts(false), Vec::new());
        let (outcome, _) = run_with(&harness, &[candidate("a")], &AtomicBool::new(false));
        assert_eq!(outcome.gate, BaselineGate::NotBlocked);
        assert!(outcome.results.is_empty());
        assert!(lock_unpoisoned(&harness.started).is_empty());
    }

    #[test]
    fn bench_records_every_candidate_and_picks_the_best() {
        let harness = FakeHarness::new(
            target_hosts(true),
            vec![
                target_hosts(false), // 100
                {
                    // 80: one discord and one youtube host fail
                    let mut hosts = target_hosts(false);
                    hosts[1].status = Status::Timeout; // d1
                    hosts[5].status = Status::Timeout; // y1
                    hosts
                },
                {
                    let mut hosts = target_hosts(false);
                    hosts[8].status = Status::Reset; // c0 breaks the internet
                    hosts
                },
            ],
        );
        let candidates = vec![candidate("good"), candidate("partial"), candidate("bad")];
        let (outcome, messages) = run_with(&harness, &candidates, &AtomicBool::new(false));

        assert_eq!(outcome.results.len(), 3);
        assert_eq!(outcome.results[0].verdict, Verdict::Works);
        assert_eq!(outcome.results[1].verdict, Verdict::Partial);
        assert_eq!(outcome.results[2].verdict, Verdict::Disqualified);
        assert_eq!(best(&outcome.results).unwrap().name, "good");
        assert!(messages.iter().any(|m| m.contains("3/3")));
        assert_eq!(lock_unpoisoned(&harness.started).len(), 3);
    }

    #[test]
    fn start_failure_is_reported_per_candidate() {
        let harness = FakeHarness::new(target_hosts(true), vec![target_hosts(false)])
            .with_start_errors(vec![Some("winws завершился".into()), None]);
        let candidates = vec![candidate("broken"), candidate("ok")];
        let (outcome, _) = run_with(&harness, &candidates, &AtomicBool::new(false));

        assert_eq!(outcome.results[0].verdict, Verdict::Broken);
        assert_eq!(
            outcome.results[0].error.as_deref(),
            Some("winws завершился")
        );
        assert_eq!(best(&outcome.results).unwrap().name, "ok");
    }

    #[test]
    fn cancel_stops_before_the_next_candidate() {
        let harness = FakeHarness::new(target_hosts(true), Vec::new());
        let cancel = AtomicBool::new(true);
        let (outcome, _) = run_with(&harness, &[candidate("a"), candidate("b")], &cancel);
        assert!(outcome.cancelled);
        assert!(outcome.results.is_empty());
        assert!(lock_unpoisoned(&harness.started).is_empty());
    }

    #[test]
    fn sort_puts_working_strategies_first() {
        let mk = |name: &str, score: u32, verdict: Verdict, ms: u64| CandidateResult {
            name: name.into(),
            source: CandidateSource::Official,
            parent: None,
            score,
            verdict,
            discord_ok: 0,
            discord_total: 4,
            youtube_ok: 0,
            youtube_total: 4,
            control_ok: 4,
            control_total: 4,
            avg_ms: ms,
            error: None,
            hosts: Vec::new(),
        };
        let mut results = vec![
            mk("broken", 20, Verdict::Broken, 10),
            mk("slow", 90, Verdict::Works, 400),
            mk("fast", 90, Verdict::Works, 100),
        ];
        sort_results(&mut results);
        assert_eq!(results[0].name, "fast");
        assert_eq!(results[1].name, "slow");
        assert_eq!(results[2].name, "broken");
    }
}
