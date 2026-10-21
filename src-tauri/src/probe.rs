use crate::sys;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

const NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };

/// Which group an endpoint belongs to. Only Discord and YouTube are scored as
/// targets; control hosts are there to notice a strategy that breaks everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    Discord,
    Youtube,
    Control,
}

/// Result of checking one host. `Unsupported` means every protocol the host was
/// asked for was rejected by the server (not by the DPI) and is scored as neutral.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    /// TLS handshake failed with a certificate problem — typical DPI substitution.
    Ssl,
    /// Connection was reset or closed mid-handshake.
    Reset,
    Timeout,
    /// Name resolution failed. This is a DNS problem, not a strategy problem.
    Dns,
    Unsupported,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub name: String,
    pub url: String,
    pub group: Group,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostProbe {
    pub name: String,
    pub url: String,
    pub group: Group,
    pub status: Status,
    pub ms: u64,
    pub detail: String,
}

/// Endpoint list shipped with the zapret runtime (`utils/targets.txt`).
/// `PING:` entries are intentionally ignored: ICMP is not what the DPI blocks,
/// so it cannot tell a working strategy from a broken one.
pub fn parse_targets(text: &str) -> Vec<Target> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Trailing comments are only recognised after whitespace, so URLs that
        // happen to contain '#' are kept intact.
        let line = match line.find(" #") {
            Some(idx) => &line[..idx],
            None => line,
        };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'').trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        if value.to_ascii_uppercase().starts_with("PING:") {
            continue;
        }
        if !(value.starts_with("https://") || value.starts_with("http://")) {
            continue;
        }
        if !seen.insert(key.to_string()) {
            continue;
        }
        out.push(Target {
            name: key.to_string(),
            url: value.to_string(),
            group: classify_group(value),
        });
    }
    out
}

pub fn classify_group(url: &str) -> Group {
    let url = url.to_ascii_lowercase();
    if url.contains("discord") {
        Group::Discord
    } else if [
        "youtube.com",
        "youtu.be",
        "ytimg",
        "googlevideo",
        "youtubei",
        "ggpht",
    ]
    .iter()
    .any(|d| url.contains(d))
    {
        Group::Youtube
    } else {
        Group::Control
    }
}

/// Mirrors `utils/targets.txt` from the zapret release. Used when the runtime is
/// missing or the file cannot be parsed, so the bench always has something to do.
pub const FALLBACK_TARGETS: &[(&str, &str)] = &[
    ("DiscordMain", "https://discord.com"),
    ("DiscordGateway", "https://gateway.discord.gg"),
    ("DiscordCDN", "https://cdn.discordapp.com"),
    ("DiscordUpdates", "https://updates.discord.com"),
    ("YouTubeWeb", "https://www.youtube.com"),
    ("YouTubeShort", "https://youtu.be"),
    ("YouTubeImage", "https://i.ytimg.com"),
    ("YouTubeVideoRedirect", "https://redirector.googlevideo.com"),
    ("GoogleMain", "https://www.google.com"),
    ("GoogleGstatic", "https://www.gstatic.com"),
    ("CloudflareWeb", "https://www.cloudflare.com"),
    ("CloudflareCDN", "https://cdnjs.cloudflare.com"),
];

pub fn fallback_targets() -> Vec<Target> {
    FALLBACK_TARGETS
        .iter()
        .map(|(name, url)| Target {
            name: name.to_string(),
            url: url.to_string(),
            group: classify_group(url),
        })
        .collect()
}

pub fn load_targets(runtime_dir: &Path) -> Vec<Target> {
    let path = runtime_dir.join("utils").join("targets.txt");
    let parsed = std::fs::read_to_string(path)
        .map(|text| parse_targets(&text))
        .unwrap_or_default();
    if parsed.iter().any(|t| t.group != Group::Control) {
        parsed
    } else {
        fallback_targets()
    }
}

/// Checks that the system `curl.exe` is usable. Without it every probe would be
/// an error and the bench would silently rank nothing.
pub fn curl_available() -> bool {
    let mut cmd = sys::command("curl.exe");
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.status().map(|status| status.success()).unwrap_or(false)
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Http11,
    Tls12,
    Tls13,
}

impl Mode {
    const ALL: [Mode; 3] = [Mode::Http11, Mode::Tls12, Mode::Tls13];

    fn label(self) -> &'static str {
        match self {
            Mode::Http11 => "http/1.1",
            Mode::Tls12 => "tls1.2",
            Mode::Tls13 => "tls1.3",
        }
    }

    fn args(self) -> &'static [&'static str] {
        match self {
            Mode::Http11 => &["--http1.1"],
            Mode::Tls12 => &["--tlsv1.2", "--tls-max", "1.2"],
            Mode::Tls13 => &["--tlsv1.3", "--tls-max", "1.3"],
        }
    }
}

pub fn probe_all(targets: &[Target], timeout: Duration) -> Vec<HostProbe> {
    let mut out = Vec::with_capacity(targets.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = targets
            .iter()
            .map(|target| scope.spawn(move || probe_host(target, timeout)))
            .collect();
        for handle in handles {
            // A panicked worker loses one host instead of injecting a fabricated
            // failure that would wrongly disqualify the whole strategy.
            if let Ok(probe) = handle.join() {
                out.push(probe);
            }
        }
    });
    out
}

pub fn probe_host(target: &Target, timeout: Duration) -> HostProbe {
    let results: Vec<(Status, u64, String)> = Mode::ALL
        .iter()
        .map(|mode| run_mode(&target.url, *mode, timeout))
        .collect();
    aggregate(target, &results)
}

fn run_mode(url: &str, mode: Mode, timeout: Duration) -> (Status, u64, String) {
    let timeout_secs = timeout.as_secs().max(1).to_string();
    let connect_secs = timeout
        .min(Duration::from_secs(2))
        .as_secs()
        .max(1)
        .to_string();

    let mut cmd = sys::command("curl.exe");
    cmd.arg("-I")
        .arg("-sS")
        .arg("--noproxy")
        .arg("*")
        .arg("-m")
        .arg(&timeout_secs)
        .arg("--connect-timeout")
        .arg(&connect_secs)
        .arg("-o")
        .arg(NULL_DEVICE)
        .arg("-w")
        .arg("%{http_code}")
        .arg("-A")
        .arg("zapret-ui/1.0")
        .args(mode.args())
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let started = Instant::now();
    match cmd.output() {
        Ok(output) => {
            let ms = started.elapsed().as_millis() as u64;
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let status = classify_result(code, &stderr);
            let detail = if stderr.is_empty() {
                format!("{}: exit {code}", mode.label())
            } else {
                format!("{}: exit {code} ({})", mode.label(), truncate(&stderr, 160))
            };
            (status, ms, detail)
        }
        Err(err) => (Status::Error, 0, format!("{}: {err}", mode.label())),
    }
}

pub(crate) fn classify_result(exit_code: i32, stderr: &str) -> Status {
    let lower = stderr.to_ascii_lowercase();

    if lower.contains("could not resolve")
        || lower.contains("resolve host")
        || lower.contains("name or service not known")
    {
        return Status::Dns;
    }
    // Windows Schannel may refuse a protocol version the way a server would; that
    // says nothing about the DPI, so it must not be scored as a failure.
    if lower.contains("not supported")
        || lower.contains("unsupported")
        || lower.contains("unknown option")
        || lower.contains("unrecognized option")
    {
        return Status::Unsupported;
    }
    if lower.contains("certificate")
        || lower.contains("self-signed")
        || lower.contains("self signed")
        || lower.contains("unable to get local issuer")
    {
        return Status::Ssl;
    }

    match exit_code {
        0 => Status::Ok,
        6 => Status::Dns,
        28 => Status::Timeout,
        35 | 52 | 56 => Status::Reset,
        51 | 60 | 77 | 83 | 90 | 91 => Status::Ssl,
        _ => Status::Error,
    }
}

/// Combines the three protocol runs into one verdict for the host.
///
/// A certificate problem wins over everything (that is what DPI substitution
/// looks like), one successful protocol is enough to call the host reachable,
/// and any hard failure outranks "unsupported".
pub fn aggregate(target: &Target, results: &[(Status, u64, String)]) -> HostProbe {
    let has = |status: Status| results.iter().any(|(s, _, _)| *s == status);
    let ok_ms = results
        .iter()
        .filter(|(s, _, _)| *s == Status::Ok)
        .map(|(_, ms, _)| *ms)
        .min();

    let status = if has(Status::Ssl) {
        Status::Ssl
    } else if ok_ms.is_some() {
        Status::Ok
    } else if has(Status::Dns) {
        Status::Dns
    } else if has(Status::Timeout) {
        Status::Timeout
    } else if has(Status::Reset) {
        Status::Reset
    } else if has(Status::Error) {
        Status::Error
    } else {
        Status::Unsupported
    };

    let detail = results
        .iter()
        .find(|(s, _, _)| *s != Status::Ok && *s != Status::Unsupported)
        .map(|(_, _, detail)| detail.clone())
        .unwrap_or_default();

    HostProbe {
        name: target.name.clone(),
        url: target.url.clone(),
        group: target.group,
        status,
        ms: if status == Status::Ok {
            ok_ms.unwrap_or(0)
        } else {
            0
        },
        detail,
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGETS_TXT: &str = r#"# targets.txt - endpoint list for zapret.ps1 tests
DiscordMain           = "https://discord.com"
DiscordGateway        = "https://gateway.discord.gg"
YouTubeWeb            = "https://www.youtube.com"
GoogleMain            = "https://www.google.com"
CloudflareDNS1111     = "PING:1.1.1.1"
Broken Line Without Equals
# AnotherDiscord = "https://example.org"
YouTubeImage          = 'https://i.ytimg.com'  # trailing comment
"#;

    #[test]
    fn parses_targets_and_skips_ping_and_comments() {
        let targets = parse_targets(TARGETS_TXT);
        let names: Vec<&str> = targets.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "DiscordMain",
                "DiscordGateway",
                "YouTubeWeb",
                "GoogleMain",
                "YouTubeImage"
            ]
        );
        assert!(
            !names.contains(&"CloudflareDNS1111"),
            "PING entries are ICMP only"
        );
        assert!(
            !names.contains(&"AnotherDiscord"),
            "commented line must be skipped"
        );
    }

    #[test]
    fn strips_quotes_and_trailing_comments() {
        let targets = parse_targets("YouTubeImage = 'https://i.ytimg.com'  # icons");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].url, "https://i.ytimg.com");
    }

    #[test]
    fn groups_hosts_by_purpose() {
        assert_eq!(classify_group("https://discord.com"), Group::Discord);
        assert_eq!(classify_group("https://cdn.discordapp.com"), Group::Discord);
        assert_eq!(classify_group("https://www.youtube.com"), Group::Youtube);
        assert_eq!(classify_group("https://youtu.be"), Group::Youtube);
        assert_eq!(classify_group("https://i.ytimg.com"), Group::Youtube);
        assert_eq!(
            classify_group("https://redirector.googlevideo.com"),
            Group::Youtube
        );
        assert_eq!(classify_group("https://www.google.com"), Group::Control);
        assert_eq!(
            classify_group("https://cdnjs.cloudflare.com"),
            Group::Control
        );
    }

    #[test]
    fn fallback_has_targets_and_controls() {
        let targets = fallback_targets();
        assert_eq!(
            targets.iter().filter(|t| t.group == Group::Discord).count(),
            4
        );
        assert_eq!(
            targets.iter().filter(|t| t.group == Group::Youtube).count(),
            4
        );
        assert_eq!(
            targets.iter().filter(|t| t.group == Group::Control).count(),
            4
        );
    }

    #[test]
    fn classifies_curl_failures() {
        assert_eq!(classify_result(0, ""), Status::Ok);
        assert_eq!(
            classify_result(6, "curl: (6) Could not resolve host: discord.com"),
            Status::Dns
        );
        assert_eq!(classify_result(28, "curl: (28) Timeout"), Status::Timeout);
        assert_eq!(
            classify_result(35, "curl: (35) error:0A000126"),
            Status::Reset
        );
        assert_eq!(
            classify_result(56, "curl: (56) Recv failure"),
            Status::Reset
        );
        assert_eq!(
            classify_result(60, "curl: (60) SSL certificate problem: self-signed"),
            Status::Ssl
        );
        // Schannel handshake failures are a failure either way; the exact label
        // only changes the diagnostic text, not the score.
        assert_eq!(
            classify_result(
                35,
                "curl: (35) schannel: next InitializeSecurityContext failed: unknown error (0x80090302)"
            ),
            Status::Reset
        );
        assert_eq!(
            classify_result(1, "curl: (1) Protocol \"tls1.3\" not supported"),
            Status::Unsupported
        );
    }

    #[test]
    fn aggregate_prefers_certificate_problem() {
        let target = Target {
            name: "DiscordMain".into(),
            url: "https://discord.com".into(),
            group: Group::Discord,
        };
        let probe = aggregate(
            &target,
            &[
                (Status::Ok, 120, String::new()),
                (Status::Ssl, 0, "tls1.3: exit 60".into()),
                (Status::Timeout, 0, String::new()),
            ],
        );
        assert_eq!(probe.status, Status::Ssl);
        assert_eq!(probe.ms, 0);
        assert!(probe.detail.contains("exit 60"));
    }

    #[test]
    fn aggregate_accepts_one_working_protocol() {
        let target = Target {
            name: "YouTubeWeb".into(),
            url: "https://www.youtube.com".into(),
            group: Group::Youtube,
        };
        let probe = aggregate(
            &target,
            &[
                (Status::Timeout, 0, String::new()),
                (Status::Ok, 210, String::new()),
                (Status::Ok, 150, String::new()),
            ],
        );
        assert_eq!(probe.status, Status::Ok);
        assert_eq!(probe.ms, 150, "fastest successful handshake wins");
    }

    #[test]
    fn aggregate_marks_all_unsupported_as_neutral() {
        let target = Target {
            name: "Control".into(),
            url: "https://example.com".into(),
            group: Group::Control,
        };
        let probe = aggregate(
            &target,
            &[
                (Status::Unsupported, 0, String::new()),
                (Status::Unsupported, 0, String::new()),
            ],
        );
        assert_eq!(probe.status, Status::Unsupported);
    }

    #[test]
    fn load_targets_falls_back_when_runtime_has_no_list() {
        let dir = std::env::temp_dir().join("zapret-ui-probe-missing-runtime");
        let targets = load_targets(&dir);
        assert!(targets.iter().any(|t| t.group == Group::Discord));
    }

    /// Manual smoke test against the real network and the real runtime targets.
    /// Run with: `cargo test live_probe_smoke -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_probe_smoke() {
        if !curl_available() {
            eprintln!("curl.exe отсутствует — пропускаем");
            return;
        }
        let targets = load_targets(&crate::config::runtime_dir());
        assert!(!targets.is_empty());
        let probes = probe_all(&targets, Duration::from_secs(4));
        for probe in &probes {
            println!(
                "{:18} {:?} {:>5}мс {}",
                probe.name, probe.status, probe.ms, probe.detail
            );
        }
        assert!(
            probes.iter().filter(|p| p.status != Status::Error).count() >= 8,
            "слишком много ошибок проверки — curl или сеть недоступны"
        );
    }
}
