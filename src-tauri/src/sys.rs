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

/// Copies UTF-16 text to the Windows clipboard.
///
/// Implemented directly instead of adding a clipboard plugin: the only thing
/// the app copies is a `tg://` link, and this keeps the dependency surface (and
/// the bundled JS) unchanged.
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let size = wide.len() * std::mem::size_of::<u16>();

    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err("Буфер обмена занят другим приложением.".into());
        }
        let _ = EmptyClipboard();
        let handle = GlobalAlloc(GMEM_MOVEABLE, size);
        if handle.is_null() {
            let _ = CloseClipboard();
            return Err("Не удалось выделить память для буфера обмена.".into());
        }
        let target = GlobalLock(handle) as *mut u16;
        if target.is_null() {
            let _ = GlobalFree(handle);
            let _ = CloseClipboard();
            return Err("Не удалось записать в буфер обмена.".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
        let _ = GlobalUnlock(handle);
        // On success the clipboard owns `handle`; freeing it here would corrupt
        // the clipboard contents.
        if SetClipboardData(CF_UNICODETEXT as u32, handle).is_null() {
            let _ = GlobalFree(handle);
            let _ = CloseClipboard();
            return Err("Не удалось записать в буфер обмена.".into());
        }
        let _ = CloseClipboard();
    }
    Ok(())
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
