use crate::sys;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Duration;

const SERVICE_NAME: &str = "zapret";

/// Manages the DPI bypass as the Windows service `zapret` — the same principle as
/// service.bat "Install Service" / "Remove Services".
pub struct Engine;

impl Engine {
    fn quiet(mut cmd: Command) -> Command {
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        cmd
    }

    /// Runs a System32 tool quietly and reports whether it exited successfully.
    fn run(name: &str, args: &[&str]) -> bool {
        Self::quiet(sys::command(name))
            .args(args)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// Install winws as the `zapret` Windows service (start=auto) and start it.
    /// Mirrors service.bat: cleanup -> sc create -> sc start -> remember strategy.
    pub fn install(exe: &str, args: &[String], strategy: &str) -> Result<(), String> {
        // Clean any previous service/driver state first.
        let _ = Self::remove();

        // Enable TCP timestamps (same as service.bat :tcp_enable).
        let _ = Self::run(
            "netsh.exe",
            &["interface", "tcp", "set", "global", "timestamps=enabled"],
        );

        // binPath value must be a single sc argument: outer quotes wrapping the
        // inner-quoted exe path and the arguments.
        let bin_path = format!("\"\\\"{exe}\\\" {}\"", args.join(" "));

        // `sc delete` is asynchronous: the service is only "marked for deletion"
        // until its last handle closes, so `sc create` can briefly fail with 1072.
        let mut created = false;
        for attempt in 0..3 {
            let mut cmd = Self::quiet(sys::command("sc.exe"));
            cmd.arg("create")
                .arg(SERVICE_NAME)
                .arg("binPath=")
                .raw_arg(&bin_path)
                .arg("DisplayName=")
                .arg("zapret")
                .arg("start=")
                .arg("auto");
            if cmd.status().map(|s| s.success()).unwrap_or(false) {
                created = true;
                break;
            }
            if attempt < 2 {
                std::thread::sleep(Duration::from_millis(400));
            }
        }
        if !created {
            return Err("Не удалось создать службу zapret.".into());
        }

        let _ = Self::run(
            "sc.exe",
            &["description", SERVICE_NAME, "Zapret DPI bypass software"],
        );

        if !Self::run("sc.exe", &["start", SERVICE_NAME]) {
            return Err("Не удалось запустить службу zapret.".into());
        }

        // Remember the strategy in the service registry key (like the bat).
        let _ = Self::run(
            "reg.exe",
            &[
                "add",
                r"HKLM\System\CurrentControlSet\Services\zapret",
                "/v",
                "zapret-discord-youtube",
                "/t",
                "REG_SZ",
                "/d",
                strategy,
                "/f",
            ],
        );
        Ok(())
    }

    /// Stop and remove the service, kill orphaned winws, and remove the WinDivert
    /// driver service (mirrors service.bat "Remove Services").
    pub fn remove() -> Result<(), String> {
        let _ = Self::run("net.exe", &["stop", SERVICE_NAME]);
        let _ = Self::run("sc.exe", &["delete", SERVICE_NAME]);

        Self::kill_by_image();

        for svc in ["WinDivert", "WinDivert14"] {
            let _ = Self::run("net.exe", &["stop", svc]);
            let _ = Self::run("sc.exe", &["delete", svc]);
        }

        // `sc delete` is asynchronous: wait until the service is really gone
        // so Stop can report failure instead of claiming success.
        for _ in 0..12 {
            if !Self::is_installed() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        if Self::is_installed() {
            return Err("Не удалось удалить службу zapret (она ещё останавливается).".into());
        }
        Ok(())
    }

    /// Whether the `zapret` service exists (running or stopped).
    pub fn is_installed() -> bool {
        let out = Self::quiet(sys::command("sc.exe"))
            .args(["query", SERVICE_NAME])
            .stdout(Stdio::piped())
            .output();
        matches!(out, Ok(output) if output.status.success())
    }

    /// Whether the `zapret` service is installed and running.
    pub fn is_running() -> bool {
        let out = Self::quiet(sys::command("sc.exe"))
            .args(["query", SERVICE_NAME])
            .stdout(Stdio::piped())
            .output();
        match out {
            Ok(output) => {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains("RUNNING")
            }
            Err(_) => false,
        }
    }

    /// Kill any winws.exe process by image name (fallback for orphaned processes).
    /// Runs synchronously: the bench has to be sure the previous candidate is gone
    /// before the next one starts.
    pub fn kill_by_image() {
        let _ = Self::run("taskkill.exe", &["/IM", "winws.exe", "/F"]);
    }

    /// Start `winws.exe` as an invisible child process with the given arguments.
    /// Used by the bench, which runs candidates without touching the service:
    /// recreating the service (and the WinDivert driver) for every candidate
    /// would be slow and fragile.
    pub fn spawn_transient(exe: &str, args: &[String]) -> Result<(), String> {
        let mut cmd = Command::new(exe);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        sys::hidden(&mut cmd);
        if let Some(dir) = std::path::Path::new(exe).parent() {
            cmd.current_dir(dir);
        }
        cmd.spawn()
            .map(|_| ())
            .map_err(|e| format!("Не удалось запустить winws.exe: {e}"))
    }

    /// Wait until any winws.exe process appears, or the timeout elapses.
    pub fn wait_winws_ready(timeout: Duration) -> bool {
        let started = std::time::Instant::now();
        while started.elapsed() < timeout {
            if Self::is_any_winws_running() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }

    /// Whether any winws.exe process is running (tasklist fallback).
    pub fn is_any_winws_running() -> bool {
        let out = Self::quiet(sys::command("tasklist.exe"))
            .args(["/FI", "IMAGENAME eq winws.exe", "/FO", "CSV", "/NH"])
            .stdout(Stdio::piped())
            .output();
        match out {
            Ok(output) => {
                let text = String::from_utf8_lossy(&output.stdout);
                text.to_lowercase().contains("winws.exe")
            }
            Err(_) => false,
        }
    }
}
