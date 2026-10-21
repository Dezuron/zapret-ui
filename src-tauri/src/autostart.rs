use crate::sys;
use std::process::Stdio;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "zapret-ui";
const OLD_TASK_NAME: &str = "zapret-ui";

fn reg_ok(args: &[&str]) -> std::io::Result<bool> {
    let status = sys::command("reg.exe")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(status.success())
}

/// Remove the old scheduled-task autostart mechanism (migration to the Run key).
fn remove_old_scheduled_task() {
    let _ = sys::command("schtasks.exe")
        .args(["/Delete", "/TN", OLD_TASK_NAME, "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// The Run-key value data: quoted exe path + the autostart flag.
fn run_value(exe: &str) -> String {
    format!("\"{exe}\" --autostart")
}

/// Register autostart via the HKCU Run key, so the app shows up in the
/// Task Manager "Startup apps" list (same mechanism as other portable apps).
pub fn install(exe: &str) -> Result<(), String> {
    remove_old_scheduled_task();

    let value = run_value(exe);
    let ok = reg_ok(&[
        "add", RUN_KEY, "/v", VALUE_NAME, "/t", "REG_SZ", "/d", &value, "/f",
    ])
    .map_err(|e| format!("reg add: {e}"))?;
    if !ok {
        return Err("Не удалось включить автозапуск.".into());
    }

    if is_installed() {
        Ok(())
    } else {
        Err("Не удалось включить автозапуск (проверка не прошла).".into())
    }
}

pub fn remove() -> Result<(), String> {
    remove_old_scheduled_task();
    let _ = reg_ok(&["delete", RUN_KEY, "/v", VALUE_NAME, "/f"]);
    if is_installed() {
        Err("Не удалось отключить автозапуск: запись Run осталась.".into())
    } else {
        Ok(())
    }
}

pub fn is_installed() -> bool {
    let out = sys::command("reg.exe")
        .args(["query", RUN_KEY, "/v", VALUE_NAME])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    matches!(out, Ok(status) if status.success())
}

/// The executable path currently registered in the Run key, if any.
pub fn registered_exe() -> Option<String> {
    let out = sys::command("reg.exe")
        .args(["query", RUN_KEY, "/v", VALUE_NAME])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_registered_exe(&String::from_utf8_lossy(&out.stdout))
}

/// Extract the exe path from a `reg query` output sample.
fn parse_registered_exe(text: &str) -> Option<String> {
    for line in text.lines() {
        if line.contains(VALUE_NAME) && line.contains("REG_SZ") {
            let data = line.split("REG_SZ").nth(1)?.trim();
            if let Some(stripped) = data.strip_prefix('"') {
                return stripped.split('"').next().map(|s| s.to_string());
            }
            return data.split_whitespace().next().map(|s| s.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{parse_registered_exe, run_value};

    #[test]
    fn run_value_quotes_exe_and_flag() {
        let exe = r"C:\Program Files\zapret-ui\zapret-ui.exe";
        assert_eq!(run_value(exe), format!("\"{exe}\" --autostart"));
    }

    #[test]
    fn parse_command_extracts_path() {
        let sample = r#"
HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run
    zapret-ui    REG_SZ    "C:\Users\Test\IdeaProjects\zapret-ui\zapret-ui.exe" --autostart
"#;
        assert_eq!(
            parse_registered_exe(sample).as_deref(),
            Some(r"C:\Users\Test\IdeaProjects\zapret-ui\zapret-ui.exe")
        );
    }

    #[test]
    fn parse_command_none_when_missing() {
        assert_eq!(parse_registered_exe("no value here"), None);
    }
}
