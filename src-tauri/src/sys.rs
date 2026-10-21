//! Windows system-command helpers.
//!
//! Every external tool is addressed by an absolute path under `%SystemRoot%`,
//! never by bare name: the app runs elevated and is portable, so a `curl.exe` or
//! `sc.exe` dropped next to the executable would otherwise be picked up by the
//! Windows search order and executed with administrator rights.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

pub const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn system32(name: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("System32").join(name)
}

/// Suppress the console window for a child process.
pub fn hidden(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

/// A `Command` for a tool in `%SystemRoot%\System32`, with no console window.
pub fn command(name: &str) -> Command {
    let mut cmd = Command::new(system32(name));
    hidden(&mut cmd);
    cmd
}

/// Opens a folder/file with the Windows shell (`ShellExecuteW`). Unlike spawning
/// `explorer.exe`, this does not depend on the tool's location and is the
/// supported way to hand a path to the shell.
pub fn open_path(path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let file: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if (result as isize) <= 32 {
        Err(format!(
            "Не удалось открыть папку (код {}).",
            result as isize
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_tools_are_absolute() {
        let path = system32("sc.exe");
        assert!(path.is_absolute(), "expected absolute path, got {path:?}");
        assert!(path
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with("system32\\sc.exe"));
    }
}
