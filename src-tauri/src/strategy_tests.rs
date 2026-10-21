#[cfg(test)]
mod tests {
    use crate::config::GameFilter;
    use crate::strategy;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    const GENERAL_BAT: &str = r#"@echo off
chcp 65001 > nul
cd /d "%~dp0"
set "BIN=%~dp0bin\"
set "LISTS=%~dp0lists\"
cd /d %BIN%
start "zapret: %~n0" /min "%BIN%winws.exe" --wf-tcp=80,443,%GameFilterTCP% --wf-udp=443,%GameFilterUDP% ^
--filter-udp=443 --hostlist="%LISTS%list-general.txt" --hostlist="%LISTS%list-general-user.txt" ^
--filter-tcp=80,443 --hostlist="%LISTS%list-general.txt" --dpi-desync=multisplit --dpi-desync-split-seqovl=568 --dpi-desync-split-pos=1 --dpi-desync-split-seqovl-pattern="%BIN%tls_clienthello_4pda_to.bin"
"#;

    const SERVICE_BAT: &str = r#"@echo off
set "LOCAL_VERSION=1.10.1"
echo menu
"#;

    fn fixture_runtime() -> std::path::PathBuf {
        let uniq = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("zapret-ui-strategy-test-{uniq}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::create_dir_all(dir.join("lists")).unwrap();
        // The runtime ships the fake/overlap ClientHello used by the Flashscore
        // fake-split profile.
        fs::write(
            dir.join("bin").join("tls_clienthello_max_ru.bin"),
            b"fake-clienthello",
        )
        .unwrap();
        fs::write(dir.join("general.bat"), GENERAL_BAT).unwrap();
        fs::write(dir.join("service.bat"), SERVICE_BAT).unwrap();
        dir
    }

    #[test]
    fn parse_general_bat() {
        let runtime = fixture_runtime();
        let args = strategy::parse_strategy_args(
            &runtime.join("general.bat"),
            &runtime,
            GameFilter::Disabled,
        )
        .expect("parse");
        assert!(args.len() > 5, "expected many args, got {}", args.len());
        // first arg should be --wf-tcp with the game filter port 12 appended
        assert!(args[0].starts_with("--wf-tcp="), "got {}", args[0]);
        assert!(args[0].ends_with(",12"), "got {}", args[0]);
        // there should be exactly one --hostlist pointing into runtime lists
        let hl = args
            .iter()
            .find(|a| a.starts_with("--hostlist="))
            .expect("hostlist");
        assert!(hl.contains("list-general.txt"), "got {}", hl);
        assert!(
            hl.contains(&runtime.to_string_lossy().to_string()),
            "got {}",
            hl
        );
        // no stray quotes
        for a in &args {
            assert!(!a.contains('"'), "quote left in arg: {a}");
        }
        // winws should not appear in args
        assert!(!args.iter().any(|a| a.contains("winws")));
        let _ = fs::remove_dir_all(&runtime);
    }

    #[test]
    fn parse_substitutes_game_filter_all() {
        let runtime = fixture_runtime();
        let args =
            strategy::parse_strategy_args(&runtime.join("general.bat"), &runtime, GameFilter::All)
                .expect("parse");
        assert!(args[0].ends_with("1024-65535"), "got {}", args[0]);
        assert!(args[1].ends_with("1024-65535"), "got {}", args[1]);
        let _ = fs::remove_dir_all(&runtime);
    }

    #[test]
    fn flashscore_section_is_inserted_before_first_filter() {
        let runtime = fixture_runtime();
        let args = strategy::parse_strategy_args(
            &runtime.join("general.bat"),
            &runtime,
            GameFilter::Disabled,
        )
        .expect("parse");
        let with = strategy::with_flashscore_section(&args, &runtime);

        // Global winws options stay in front of the injected profile.
        assert_eq!(with[0], args[0]);
        assert_eq!(with[1], args[1]);

        // The flashscore profiles become the first two filter profiles.
        assert_eq!(with[2], "--filter-tcp=80,443");
        assert_eq!(
            with[3],
            "--hostlist-domains=flashscorekz.com,flashscore.com.ua,flashscore.ninja"
        );
        assert!(with.contains(&"--dpi-desync=multidisorder".to_string()));
        assert!(with.contains(&"--dpi-desync-split-pos=1,midsld".to_string()));
        assert!(with.contains(&"--dpi-desync-fooling=ts".to_string()));
        assert!(with.contains(&"--dpi-desync-repeats=4".to_string()));
        assert!(
            !with.iter().any(|a| a.contains("hostfakesplit")),
            "hostfakesplit is RST-ed by the current DPI"
        );

        // Second profile: the Cloudflare/Google hosts (logos, flags, icons).
        assert!(with.contains(&"--hostlist-domains=flashscore.com".to_string()));
        assert!(with.contains(&"--dpi-desync=fake,multisplit".to_string()));
        assert!(with.contains(&"--dpi-desync-split-seqovl=664".to_string()));
        assert!(with.contains(&"--dpi-desync-repeats=8".to_string()));
        let pattern = with
            .iter()
            .find(|a| a.starts_with("--dpi-desync-split-seqovl-pattern="))
            .expect("seqovl pattern");
        assert!(
            pattern.contains("tls_clienthello_max_ru.bin"),
            "got {pattern}"
        );
        assert!(
            pattern.contains(&runtime.to_string_lossy().to_string()),
            "pattern path must point into runtime: {pattern}"
        );
        let fake = with
            .iter()
            .find(|a| a.starts_with("--dpi-desync-fake-tls="))
            .expect("fake tls");
        assert!(fake.contains("tls_clienthello_max_ru.bin"), "got {fake}");

        let excludes: Vec<&String> = with
            .iter()
            .filter(|a| a.starts_with("--hostlist-exclude="))
            .collect();
        assert_eq!(excludes.len(), 4, "two exclude lists per profile expected");
        for e in excludes {
            assert!(
                e.contains(&runtime.to_string_lossy().to_string()),
                "exclude path must point into runtime: {e}"
            );
        }

        // The injected profile ends with --new, and the original filter chain survives.
        let orig_filter = args
            .iter()
            .position(|a| a.starts_with("--filter-"))
            .expect("original filter");
        let tail_start = with
            .iter()
            .skip(2)
            .position(|a| a.starts_with("--filter-") && a != "--filter-tcp=80,443")
            .map(|i| i + 2)
            .expect("original filter after injection");
        assert_eq!(with[tail_start - 1], "--new");
        assert_eq!(&with[tail_start..], &args[orig_filter..]);

        for a in &with {
            assert!(!a.contains('"'), "quote left in arg: {a}");
        }
        let _ = fs::remove_dir_all(&runtime);
    }

    #[test]
    fn flashscore_section_appended_when_no_filter_exists() {
        let runtime = fixture_runtime();
        let args = vec!["--wf-tcp=80,443".to_string()];
        let with = strategy::with_flashscore_section(&args, &runtime);

        assert_eq!(with[0], "--wf-tcp=80,443");
        assert_eq!(with[1], "--new");
        assert_eq!(with[2], "--filter-tcp=80,443");
        assert_eq!(
            with[3],
            "--hostlist-domains=flashscorekz.com,flashscore.com.ua,flashscore.ninja"
        );
        // Both profiles are injected and the section ends with the fake TLS record.
        let second = with
            .iter()
            .position(|a| a == "--hostlist-domains=flashscore.com")
            .expect("second (fake-split) profile");
        assert_eq!(with[second - 1], "--filter-tcp=80,443");
        assert!(
            with.last()
                .map(|a| a.starts_with("--dpi-desync-fake-tls="))
                .unwrap_or(false),
            "section must end with the fake TLS record"
        );
        let _ = fs::remove_dir_all(&runtime);
    }

    #[test]
    fn flashscore_falls_back_to_reorder_without_pattern_file() {
        let runtime = fixture_runtime();
        fs::remove_file(runtime.join("bin").join("tls_clienthello_max_ru.bin")).unwrap();
        let args = vec!["--wf-tcp=80,443".to_string()];
        let with = strategy::with_flashscore_section(&args, &runtime);

        // The com hosts are still covered, but without referencing the missing file:
        // winws refuses to start on a missing desync file.
        assert!(with.contains(&"--hostlist-domains=flashscore.com".to_string()));
        assert!(!with
            .iter()
            .any(|a| a.contains("tls_clienthello_max_ru.bin")));
        assert!(!with.iter().any(|a| a.starts_with("--dpi-desync-fake-tls=")));
        assert_eq!(
            with.iter()
                .filter(|a| a.as_str() == "--dpi-desync=multidisorder")
                .count(),
            2,
            "both profiles should fall back to the reorder desync"
        );
        let _ = fs::remove_dir_all(&runtime);
    }

    #[test]
    fn discover_skips_service_and_sorts() {
        let runtime = fixture_runtime();
        fs::write(
            runtime.join("general (ALT).bat"),
            GENERAL_BAT
                .replace("%GameFilterTCP%", "12")
                .replace("%GameFilterUDP%", "12"),
        )
        .unwrap();
        fs::write(
            runtime.join("general (ALT2).bat"),
            GENERAL_BAT
                .replace("%GameFilterTCP%", "12")
                .replace("%GameFilterUDP%", "12"),
        )
        .unwrap();

        let list = strategy::discover_strategies(&runtime, GameFilter::Disabled);
        // service.bat must be skipped
        assert!(
            !list.iter().any(|s| s.name == "service"),
            "service.bat must not be listed"
        );
        assert!(list.iter().any(|s| s.name == "general"));
        // natural sort: general (ALT) before general (ALT2)
        assert_eq!(list[0].name, "general");
        assert_eq!(list[1].name, "general (ALT)");
        assert_eq!(list[2].name, "general (ALT2)");
        let _ = fs::remove_dir_all(&runtime);
    }
}
