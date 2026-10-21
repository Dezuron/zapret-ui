use crate::config;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Values that appear in the official strategies; used as a safe crossover pool.
const SPLIT_POS_VALUES: &[&str] = &[
    "1",
    "2",
    "3",
    "1,midsld",
    "midsld",
    "1,midsld,1",
    "1,2",
    "5",
    "1,1",
];
const FOOLING_VALUES: &[&str] = &[
    "ts",
    "md5sig",
    "ts,md5sig",
    "badseq",
    "datanoack",
    "ts,badseq",
    "md5sig,badseq",
];
const REPEATS_VALUES: &[&str] = &["2", "3", "4", "6", "8", "10", "12"];
const SEQOVL_VALUES: &[&str] = &["1", "64", "336", "568", "664", "1024"];
const TTL_VALUES: &[&str] = &["1", "2", "3", "4", "5", "6", "7", "8"];

const PATH_KEYS: &[&str] = &[
    "--hostlist",
    "--hostlist-exclude",
    "--ipset",
    "--ipset-exclude",
    "--dpi-desync-fake-tls",
    "--dpi-desync-fake-quic",
    "--dpi-desync-fake-http",
    "--dpi-desync-fake-syndata",
    "--dpi-desync-split-seqovl-pattern",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedStrategy {
    pub name: String,
    pub parent: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub mutation: String,
    #[serde(default)]
    pub score: Option<u32>,
    pub created_at: u64,
}

#[derive(Debug, Clone)]
pub struct Seed {
    pub name: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MutKind {
    SplitPos,
    Fooling,
    Repeats,
    SeqOvl,
    Ttl,
    Desync,
    FakeFile,
}

fn mutable_kind(key: &str) -> Option<MutKind> {
    match key {
        "--dpi-desync-split-pos" => Some(MutKind::SplitPos),
        "--dpi-desync-fooling" => Some(MutKind::Fooling),
        "--dpi-desync-repeats" => Some(MutKind::Repeats),
        "--dpi-desync-split-seqovl" => Some(MutKind::SeqOvl),
        "--dpi-desync-ttl" => Some(MutKind::Ttl),
        "--dpi-desync" => Some(MutKind::Desync),
        "--dpi-desync-fake-tls"
        | "--dpi-desync-fake-quic"
        | "--dpi-desync-fake-http"
        | "--dpi-desync-fake-syndata"
        | "--dpi-desync-split-seqovl-pattern" => Some(MutKind::FakeFile),
        _ => None,
    }
}

fn fake_file_prefix(key: &str) -> Option<&'static str> {
    match key {
        // The sequence-overlap pattern is usually another TLS ClientHello, which is
        // what the runtime ships. HTTP/syndata payloads have no matching `.bin`
        // family in the release, so those keys are left alone rather than being
        // filled with an unrelated TLS record.
        "--dpi-desync-fake-tls" | "--dpi-desync-split-seqovl-pattern" => Some("tls_clienthello"),
        "--dpi-desync-fake-quic" => Some("quic_initial"),
        _ => None,
    }
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

pub fn generate(
    seeds: &[Seed],
    budget: usize,
    runtime_dir: &Path,
    rng_seed: u64,
    reserved: &HashSet<String>,
) -> Vec<GeneratedStrategy> {
    if seeds.is_empty() || budget == 0 {
        return Vec::new();
    }

    let mut rng = Rng::new(rng_seed);
    let desync_values: Vec<String> = {
        let mut set = HashSet::new();
        for seed in seeds {
            for arg in &seed.args {
                if let Some(value) = arg.strip_prefix("--dpi-desync=") {
                    set.insert(value.to_string());
                }
            }
        }
        let mut values: Vec<String> = set.into_iter().collect();
        values.sort();
        values
    };

    let mut used: HashSet<String> = seeds.iter().map(|s| key(&s.args)).collect();
    let mut taken: HashSet<String> = reserved.clone();
    let mut out = Vec::new();
    let mut attempts = 0usize;
    let max_attempts = budget.saturating_mul(10).max(10);

    while out.len() < budget && attempts < max_attempts {
        attempts += 1;
        let seed = &seeds[rng.below(seeds.len())];
        let mut args = seed.args.clone();
        let descriptions = mutate(&mut args, &mut rng, &desync_values, runtime_dir);
        if descriptions.is_empty() {
            continue;
        }
        if validate_args(&args).is_err() {
            continue;
        }
        if !used.insert(key(&args)) {
            continue;
        }
        let name = next_name(&mut taken);
        out.push(GeneratedStrategy {
            name,
            parent: seed.name.clone(),
            args,
            mutation: descriptions.join("; "),
            score: None,
            created_at: crate::bench::now(),
        });
    }
    out
}

fn next_name(taken: &mut HashSet<String>) -> String {
    let mut n = 1;
    loop {
        let name = format!("auto-{n}");
        if taken.insert(name.clone()) {
            return name;
        }
        n += 1;
    }
}

fn mutate(
    args: &mut [String],
    rng: &mut Rng,
    desync_values: &[String],
    runtime_dir: &Path,
) -> Vec<String> {
    let mutable: Vec<usize> = args
        .iter()
        .enumerate()
        .filter(|(_, token)| {
            token
                .split_once('=')
                .map(|(key, _)| mutable_kind(key).is_some())
                .unwrap_or(false)
        })
        .map(|(index, _)| index)
        .collect();
    if mutable.is_empty() {
        return Vec::new();
    }

    let changes = if mutable.len() > 1 && rng.below(3) == 0 {
        2
    } else {
        1
    };
    let mut touched = HashSet::new();
    let mut descriptions = Vec::new();

    for _ in 0..changes {
        let index = mutable[rng.below(mutable.len())];
        if !touched.insert(index) {
            continue;
        }
        let Some((key, old)) = args[index].split_once('=') else {
            continue;
        };
        let key = key.to_string();
        let old = old.to_string();
        let Some(kind) = mutable_kind(&key) else {
            continue;
        };
        let new_value = match kind {
            MutKind::SplitPos => Some(rng.pick(SPLIT_POS_VALUES).to_string()),
            MutKind::Fooling => Some(rng.pick(FOOLING_VALUES).to_string()),
            MutKind::Repeats => Some(rng.pick(REPEATS_VALUES).to_string()),
            MutKind::SeqOvl => Some(rng.pick(SEQOVL_VALUES).to_string()),
            MutKind::Ttl => Some(rng.pick(TTL_VALUES).to_string()),
            MutKind::Desync => {
                if desync_values.len() > 1 {
                    Some(rng.pick(desync_values).to_string())
                } else {
                    None
                }
            }
            MutKind::FakeFile => {
                let files = fake_file_prefix(&key)
                    .map(|prefix| fake_files(runtime_dir, prefix))
                    .unwrap_or_default();
                if files.len() > 1 {
                    Some(rng.pick(&files).to_string())
                } else {
                    None
                }
            }
        };
        let Some(new_value) = new_value else {
            continue;
        };
        if new_value == old {
            continue;
        }
        args[index] = format!("{key}={new_value}");
        descriptions.push(format!(
            "{}: {} → {}",
            key.trim_start_matches("--"),
            old,
            new_value
        ));
    }
    descriptions
}

fn fake_files(runtime_dir: &Path, prefix: &str) -> Vec<String> {
    let bin = runtime_dir.join("bin");
    let Ok(entries) = std::fs::read_dir(&bin) else {
        return Vec::new();
    };
    let mut files: Vec<String> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|e| e.to_str()) == Some("bin")
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|name| name.to_ascii_lowercase().starts_with(prefix))
                    .unwrap_or(false)
        })
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    files.sort();
    files
}

pub fn validate_args(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("Пустые аргументы стратегии.".into());
    }
    if args.first().map(|a| a == "--new").unwrap_or(false)
        || args.last().map(|a| a == "--new").unwrap_or(false)
    {
        return Err("Некорректная структура профилей (--new).".into());
    }
    if !args.iter().any(|a| a.starts_with("--filter-")) {
        return Err("В стратегии нет ни одного профиля (--filter-).".into());
    }
    let first_new = args.iter().position(|a| a == "--new").unwrap_or(args.len());
    for (index, arg) in args.iter().enumerate() {
        if arg.contains('"') {
            return Err(format!("Кавычка в аргументе: {arg}"));
        }
        if arg.starts_with("--wf-") && index > first_new {
            return Err("Глобальный --wf-* оказался внутри профиля.".into());
        }
        let Some((key, value)) = arg.split_once('=') else {
            continue;
        };
        if PATH_KEYS.contains(&key) && !value.is_empty() && !Path::new(value).exists() {
            return Err(format!("Файл не найден: {value}"));
        }
    }
    Ok(())
}

pub fn key(args: &[String]) -> String {
    args.join("\u{1f}")
}

// ---------- persistence ----------

pub fn generated_path() -> PathBuf {
    config::app_dir().join("generated.json")
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct GeneratedFile {
    #[serde(default)]
    strategies: Vec<GeneratedStrategy>,
}

pub fn load_generated_from(path: &Path) -> Vec<GeneratedStrategy> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<GeneratedFile>(&text)
        .map(|file| file.strategies)
        .unwrap_or_default()
}

pub fn save_generated_to(path: &Path, list: &[GeneratedStrategy]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let file = GeneratedFile {
        strategies: list.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn load_generated() -> Vec<GeneratedStrategy> {
    load_generated_from(&generated_path())
}

pub fn find_generated(name: &str) -> Option<GeneratedStrategy> {
    load_generated().into_iter().find(|s| s.name == name)
}

pub fn delete_generated(name: &str) -> Result<(), String> {
    let mut list = load_generated();
    let before = list.len();
    list.retain(|s| s.name != name);
    if list.len() == before {
        return Err(format!("Стратегия «{name}» не найдена."));
    }
    save_generated_to(&generated_path(), &list)
}

/// Appends new winners, skipping candidates whose arguments already exist.
/// Existing strategies are kept so a search never wipes earlier results.
pub fn merge_winners_at(
    path: &Path,
    winners: Vec<GeneratedStrategy>,
) -> Result<Vec<GeneratedStrategy>, String> {
    let mut list = load_generated_from(path);
    let mut known: HashSet<String> = list.iter().map(|s| key(&s.args)).collect();
    let mut names: HashSet<String> = list.iter().map(|s| s.name.clone()).collect();
    let mut added = Vec::new();

    for mut winner in winners {
        if !known.insert(key(&winner.args)) {
            continue;
        }
        // Keep the name the search scored and applied; only rename on a clash,
        // otherwise `auto-3` would be saved as `auto-1` and could no longer be
        // resolved when the summary says it was applied.
        if !names.insert(winner.name.clone()) {
            winner.name = next_name(&mut names);
        }
        added.push(winner.clone());
        list.push(winner);
    }

    save_generated_to(path, &list)?;
    Ok(added)
}

pub fn merge_winners(winners: Vec<GeneratedStrategy>) -> Result<Vec<GeneratedStrategy>, String> {
    merge_winners_at(&generated_path(), winners)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_runtime(name: &str) -> PathBuf {
        let uniq = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("zapret-ui-gen-{name}-{uniq}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::create_dir_all(dir.join("lists")).unwrap();
        for file in [
            "tls_clienthello_a.bin",
            "tls_clienthello_b.bin",
            "quic_initial_a.bin",
            "quic_initial_b.bin",
        ] {
            fs::write(dir.join("bin").join(file), b"payload").unwrap();
        }
        fs::write(dir.join("lists").join("list-general.txt"), "").unwrap();
        dir
    }

    fn seed(runtime: &Path) -> Seed {
        let bin = runtime.join("bin");
        let lists = runtime.join("lists");
        Seed {
            name: "general (ALT7)".into(),
            args: vec![
                "--wf-tcp=80,443".to_string(),
                "--wf-udp=443".to_string(),
                "--filter-tcp=443".to_string(),
                format!(
                    "--hostlist={}",
                    lists.join("list-general.txt").to_string_lossy()
                ),
                "--dpi-desync=fake,split".to_string(),
                "--dpi-desync-split-pos=1".to_string(),
                "--dpi-desync-fooling=ts".to_string(),
                "--dpi-desync-repeats=4".to_string(),
                format!(
                    "--dpi-desync-fake-tls={}",
                    bin.join("tls_clienthello_a.bin").to_string_lossy()
                ),
                "--new".to_string(),
                "--filter-udp=443".to_string(),
                "--dpi-desync=fake".to_string(),
                format!(
                    "--dpi-desync-fake-quic={}",
                    bin.join("quic_initial_a.bin").to_string_lossy()
                ),
            ],
        }
    }

    #[test]
    fn generates_valid_unique_candidates() {
        let runtime = fixture_runtime("valid");
        let seeds = vec![seed(&runtime)];
        let generated = generate(&seeds, 15, &runtime, 42, &HashSet::new());

        assert_eq!(generated.len(), 15, "should reach the budget");
        let mut keys = HashSet::new();
        for candidate in &generated {
            assert!(keys.insert(key(&candidate.args)), "duplicate candidate");
            assert!(
                validate_args(&candidate.args).is_ok(),
                "invalid candidate: {:?}",
                validate_args(&candidate.args)
            );
            assert_eq!(candidate.parent, "general (ALT7)");
            assert!(!candidate.mutation.is_empty());
            assert!(candidate.name.starts_with("auto-"));
        }

        // Filters and global options must survive untouched.
        let filtered: usize = generated
            .iter()
            .map(|c| c.args.iter().filter(|a| a.starts_with("--filter-")).count())
            .min()
            .unwrap();
        assert_eq!(filtered, 2, "both profiles must remain");
        for candidate in &generated {
            assert_eq!(candidate.args[0], "--wf-tcp=80,443");
            assert_eq!(candidate.args[1], "--wf-udp=443");
            assert!(!candidate.args.iter().any(|a| a.contains('"')));
        }

        let _ = fs::remove_dir_all(&runtime);
    }

    #[test]
    fn generation_is_deterministic_for_the_same_seed() {
        let runtime = fixture_runtime("determinism");
        let seeds = vec![seed(&runtime)];
        let a = generate(&seeds, 8, &runtime, 7, &HashSet::new());
        let b = generate(&seeds, 8, &runtime, 7, &HashSet::new());
        assert_eq!(
            a.iter().map(|s| s.args.clone()).collect::<Vec<_>>(),
            b.iter().map(|s| s.args.clone()).collect::<Vec<_>>()
        );
        let _ = fs::remove_dir_all(&runtime);
    }

    #[test]
    fn no_mutable_keys_means_no_candidates() {
        let runtime = fixture_runtime("nomutable");
        let seeds = vec![Seed {
            name: "flat".into(),
            args: vec!["--wf-tcp=80,443".into(), "--filter-tcp=443".into()],
        }];
        assert!(generate(&seeds, 5, &runtime, 1, &HashSet::new()).is_empty());
        let _ = fs::remove_dir_all(&runtime);
    }

    #[test]
    fn validate_rejects_missing_desync_file() {
        let runtime = fixture_runtime("missing");
        let bin = runtime.join("bin");
        let lists = runtime.join("lists");
        let args = vec![
            "--wf-tcp=80,443".to_string(),
            "--filter-tcp=443".to_string(),
            format!(
                "--hostlist={}",
                lists.join("list-general.txt").to_string_lossy()
            ),
            format!(
                "--dpi-desync-fake-tls={}",
                bin.join("nope.bin").to_string_lossy()
            ),
        ];
        assert!(validate_args(&args).is_err());
    }

    #[test]
    fn validate_rejects_wf_inside_profile() {
        let args = vec![
            "--filter-tcp=443".to_string(),
            "--new".to_string(),
            "--wf-tcp=80,443".to_string(),
        ];
        assert!(validate_args(&args).is_err());
    }

    #[test]
    fn merge_keeps_existing_and_dedupes_by_args() {
        let dir = std::env::temp_dir().join(format!(
            "zapret-ui-gen-merge-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("generated.json");

        let winner = |name: &str, args: &[&str]| GeneratedStrategy {
            name: name.into(),
            parent: "seed".into(),
            args: args.iter().map(|a| a.to_string()).collect(),
            mutation: "test".into(),
            score: Some(100),
            created_at: 1,
        };

        let added = merge_winners_at(&path, vec![winner("auto-1", &["--filter-tcp=443"])]).unwrap();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].name, "auto-1");

        // Same arguments again: skipped. A different candidate gets a free name.
        let added = merge_winners_at(
            &path,
            vec![
                winner("auto-1", &["--filter-tcp=443"]),
                winner("auto-1", &["--filter-tcp=80"]),
            ],
        )
        .unwrap();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].name, "auto-2");
        assert_eq!(load_generated_from(&path).len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_keeps_a_free_winner_name() {
        let dir = std::env::temp_dir().join(format!(
            "zapret-ui-gen-keep-name-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("generated.json");

        let winner = GeneratedStrategy {
            name: "auto-7".into(),
            parent: "seed".into(),
            args: vec!["--filter-tcp=443".into()],
            mutation: "test".into(),
            score: Some(100),
            created_at: 1,
        };
        let added = merge_winners_at(&path, vec![winner]).unwrap();
        assert_eq!(added[0].name, "auto-7", "scored name must survive saving");
        assert_eq!(load_generated_from(&path)[0].name, "auto-7");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn keeps_http_fake_payload_untouched() {
        let runtime = fixture_runtime("http");
        let http = runtime.join("bin").join("http_payload.bin");
        fs::write(&http, b"http").unwrap();

        let mut seed = seed(&runtime);
        seed.args
            .push(format!("--dpi-desync-fake-http={}", http.to_string_lossy()));

        let generated = generate(&[seed], 12, &runtime, 3, &HashSet::new());
        assert!(!generated.is_empty());
        let expected = format!("--dpi-desync-fake-http={}", http.to_string_lossy());
        for candidate in &generated {
            assert!(
                candidate.args.contains(&expected),
                "HTTP payload must not be replaced with a TLS record: {:?}",
                candidate.args
            );
        }

        let _ = fs::remove_dir_all(&runtime);
    }
}
