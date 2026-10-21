use crate::config::GameFilter;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Strategy {
    pub name: String,
    pub file_name: String,
    pub path: PathBuf,
    pub args: Vec<String>,
}

pub fn discover_strategies(runtime_dir: &Path, game_filter: GameFilter) -> Vec<Strategy> {
    let mut out: Vec<Strategy> = Vec::new();
    let Ok(entries) = fs::read_dir(runtime_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".bat") || file_name.to_lowercase().starts_with("service") {
            continue;
        }
        if let Some(args) = parse_strategy_args(&path, runtime_dir, game_filter) {
            out.push(Strategy {
                name: file_name.trim_end_matches(".bat").to_string(),
                file_name: file_name.to_string(),
                path,
                args,
            });
        }
    }
    out.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    out
}

/// Flashscore hosts served from the project's own nginx (`flashscorekz.com`).
/// The DPI answers every `fake`-based profile here with an immediate RST, so only
/// a reordering multi-split opens them; `flashscore.ninja` is not blocked at all
/// but survives the same profile unharmed.
pub const FLASHSCORE_REORDER_DOMAINS: &str = "flashscorekz.com,flashscore.com.ua,flashscore.ninja";

/// Flashscore hosts served from Google Cloud (`www.flashscore.com`) and Cloudflare
/// (`static.flashscore.com` — every logo, flag and icon). A reordering multi-split
/// opens the handshake but lets concurrent HTTP/2 streams on these hosts collapse
/// mid-flight, so they get a fake split with a sequence-overlap pattern instead.
pub const FLASHSCORE_FAKE_SPLIT_DOMAINS: &str = "flashscore.com";

/// ClientHello of an unrelated site, shipped by the zapret runtime in `bin/`. It is
/// used as both the fake TLS record and the sequence-overlap pattern: the DPI
/// reassembles a harmless SNI while the server still receives the real request.
const FAKE_TLS_CLIENTHELLO: &str = "tls_clienthello_max_ru.bin";

/// Injects highest-priority winws profiles for the Flashscore hosts so they can be
/// combined with any updatable strategy. The zone is blocked by SNI, but the hosts
/// sit behind different stacks and need different desyncs, so two profiles are
/// injected: a reordering multi-split for the nginx hosts and a fake split for the
/// Cloudflare/Google ones (see the constants above for the measurements behind it).
pub fn with_flashscore_section(args: &[String], runtime_dir: &Path) -> Vec<String> {
    let lists = runtime_dir.join("lists");
    let bin = runtime_dir.join("bin");
    let excludes: Vec<String> = ["list-exclude.txt", "list-exclude-user.txt"]
        .iter()
        .map(|name| format!("--hostlist-exclude={}", lists.join(name).to_string_lossy()))
        .collect();

    let reorder_profile = [
        "--dpi-desync=multidisorder".to_string(),
        "--dpi-desync-split-pos=1,midsld".to_string(),
        "--dpi-desync-fooling=ts".to_string(),
        "--dpi-desync-repeats=4".to_string(),
    ];
    let pattern = bin.join(FAKE_TLS_CLIENTHELLO);
    let pattern_path = pattern.to_string_lossy().to_string();
    let fake_split_profile = if pattern.exists() {
        vec![
            "--dpi-desync=fake,multisplit".to_string(),
            "--dpi-desync-split-pos=1".to_string(),
            "--dpi-desync-split-seqovl=664".to_string(),
            "--dpi-desync-fooling=ts".to_string(),
            "--dpi-desync-repeats=8".to_string(),
            format!("--dpi-desync-split-seqovl-pattern={pattern_path}"),
            format!("--dpi-desync-fake-tls={pattern_path}"),
        ]
    } else {
        // winws exits when a referenced desync file is missing, which would take the
        // whole bypass down. Fall back to the file-free reorder profile instead.
        reorder_profile.to_vec()
    };

    let mut section = vec![
        "--filter-tcp=80,443".to_string(),
        format!("--hostlist-domains={FLASHSCORE_REORDER_DOMAINS}"),
    ];
    section.extend(excludes.iter().cloned());
    section.extend(reorder_profile);
    section.push("--new".to_string());
    section.push("--filter-tcp=80,443".to_string());
    section.push(format!(
        "--hostlist-domains={FLASHSCORE_FAKE_SPLIT_DOMAINS}"
    ));
    section.extend(excludes);
    section.extend(fake_split_profile);

    match args.iter().position(|a| a.starts_with("--filter-")) {
        Some(idx) => {
            let mut out = Vec::with_capacity(args.len() + section.len() + 1);
            out.extend_from_slice(&args[..idx]);
            out.append(&mut section);
            out.push("--new".to_string());
            out.extend_from_slice(&args[idx..]);
            out
        }
        None => {
            let mut out = args.to_vec();
            if !out.is_empty() {
                out.push("--new".to_string());
            }
            out.append(&mut section);
            out
        }
    }
}

pub fn parse_strategy_args(
    bat_path: &Path,
    runtime_dir: &Path,
    game_filter: GameFilter,
) -> Option<Vec<String>> {
    let content = fs::read_to_string(bat_path).ok()?;
    let joined = join_continuations(&content);

    let start = joined.find("winws.exe")?;
    let mut rest = joined[start + "winws.exe".len()..].to_string();
    // strip the closing quote of the path and following whitespace
    rest = rest.trim_start_matches('"').trim_start().to_string();

    // Match bat semantics: BIN/LISTS/%~dp0 end with a trailing backslash.
    let bin = format!("{}\\", runtime_dir.join("bin").to_string_lossy());
    let lists = format!("{}\\", runtime_dir.join("lists").to_string_lossy());
    let root = format!("{}\\", runtime_dir.to_string_lossy());

    let mut expanded = rest
        .replace("%BIN%", &bin)
        .replace("%LISTS%", &lists)
        .replace("%GameFilterTCP%", game_filter.tcp_value())
        .replace("%GameFilterUDP%", game_filter.udp_value())
        .replace("%~dp0", &root)
        .replace("^!", "!")
        .replace("^^", "^");

    // cmd line continuations: a trailing '^' followed by nothing continues next line.
    // handled in join_continuations already; remaining '^' escapes are resolved above.

    expanded = expanded.trim().to_string();
    Some(tokenize(&expanded))
}

/// Join lines where a line ends with `^` (cmd line continuation).
fn join_continuations(content: &str) -> String {
    let mut out = String::new();
    for line in content.lines() {
        let trimmed_end = line.trim_end();
        let text = trimmed_end.strip_suffix('^').unwrap_or(line);
        out.push_str(text);
        out.push(' ');
    }
    out
}

/// Tokenize respecting double quotes; strip quote chars, keep `key=value` intact.
fn tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for c in s.chars() {
        match c {
            '"' => in_quote = !in_quote,
            ' ' | '\t' if !in_quote => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ca = a.chars().peekable();
    let mut cb = b.chars().peekable();
    loop {
        match (ca.peek(), cb.peek()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(&x), Some(&y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na: String = ca.by_ref().take_while(|c| c.is_ascii_digit()).collect();
                let nb: String = cb.by_ref().take_while(|c| c.is_ascii_digit()).collect();
                let (a0, b0) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = a0.len().cmp(&b0.len());
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
                let ord = a0.cmp(b0);
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            (Some(&x), Some(&y)) => {
                let ord = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
                ca.next();
                cb.next();
            }
        }
    }
}
