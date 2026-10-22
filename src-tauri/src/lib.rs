mod autostart;
mod bench;
mod commands;
mod config;
mod engine;
mod error;
mod generator;
mod probe;
mod runtime;
mod strategy;
mod sys;
mod tgws;
mod updater;
mod whats_new;

#[cfg(test)]
mod bench_live_tests;
#[cfg(test)]
mod strategy_tests;

use bench::BenchSummary;
use config::AppConfig;
pub use error::{AppError, AppResult};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Locks a mutex even if another thread panicked while holding it. A poisoned
/// config mutex must not turn every later command into a panic: the value is
/// still readable/writable, and the app should keep running.
pub fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn lock_config(state: &AppState) -> std::sync::MutexGuard<'_, AppConfig> {
    lock_unpoisoned(&state.config)
}

pub struct AppState {
    pub config: Mutex<AppConfig>,
    /// True while a benchmark or search is running; guards concurrent runs.
    pub bench_running: Arc<AtomicBool>,
    /// True while the runtime is being updated.
    pub update_running: Arc<AtomicBool>,
    /// Set by `cancel_bench`, checked between candidates.
    pub bench_cancel: Arc<AtomicBool>,
    /// Last benchmark of this session (persisted to bench.json as well).
    pub last_bench: Mutex<Option<BenchSummary>>,
    /// Supervisor for the bundled Telegram MTProto proxy.
    pub tgws: crate::tgws::Tgws,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(config::load_config()),
            bench_running: Arc::new(AtomicBool::new(false)),
            update_running: Arc::new(AtomicBool::new(false)),
            bench_cancel: Arc::new(AtomicBool::new(false)),
            last_bench: Mutex::new(None),
            tgws: crate::tgws::Tgws::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
fn is_admin() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

/// Relaunch elevated (UAC) when the app is started normally but lacks admin rights.
///
/// Uses `ShellExecuteExW` with the "runas" verb directly instead of a PowerShell
/// subprocess: no console window flashes and the elevation behaves exactly like
/// "Run as administrator", which works regardless of the UAC prompt policy.
fn ensure_elevated() {
    #[cfg(not(debug_assertions))]
    {
        use windows_sys::Win32::UI::Shell::{
            ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

        if is_admin() {
            return;
        }

        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let verb: Vec<u16> = "runas\0".encode_utf16().collect();
        let file: Vec<u16> = exe
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let params: Vec<u16> = std::env::args()
            .skip(1)
            .collect::<Vec<_>>()
            .join(" ")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = if params.is_empty() {
            std::ptr::null()
        } else {
            params.as_ptr()
        };
        info.nShow = 1; // SW_SHOWNORMAL

        let ok = unsafe { ShellExecuteExW(&mut info) };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            let msg: Vec<u16> = format!(
                "Не удалось запустить zapret-ui с правами администратора.\n\nОшибка: {err}\n\nЗапустите приложение через «Запуск от имени администратора»."
            )
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
            let title: Vec<u16> = "zapret-ui"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            unsafe {
                MessageBoxW(
                    std::ptr::null_mut(),
                    msg.as_ptr(),
                    title.as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        std::process::exit(0);
    }
}

/// Ask DWM for a dark or light native title bar. The non-client caption is
/// drawn by Windows; the app theme only changes the webview unless this
/// attribute is kept in sync.
pub fn apply_titlebar_theme(window: &tauri::WebviewWindow, dark: bool) {
    use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};

    let _ = window.set_theme(Some(if dark {
        tauri::Theme::Dark
    } else {
        tauri::Theme::Light
    }));

    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let enabled: i32 = if dark { 1 } else { 0 };
    let caption: u32 = if dark { 0x000d0909 } else { 0x00f4f0ee };
    let caption_text: u32 = if dark { 0x00f6f3f2 } else { 0x00221917 };
    unsafe {
        DwmSetWindowAttribute(
            hwnd.0,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &enabled as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );
        // Windows 11: also paint the native caption explicitly, otherwise a
        // system dark theme can keep a dark title bar over a light app.
        DwmSetWindowAttribute(
            hwnd.0,
            35_u32,
            &caption as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
        DwmSetWindowAttribute(
            hwnd.0,
            36_u32,
            &caption_text as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

/// Reads `--name value` from the process arguments.
fn arg_value(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}

/// Waits for the previous app process to exit. The updater starts the new exe
/// before the old one terminates; the single-instance plugin must not run until
/// the old instance is gone.
fn wait_for_process_exit(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};

    const SYNCHRONIZE: u32 = 0x0010_0000;
    let handle = unsafe { OpenProcess(SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        return;
    }
    unsafe {
        WaitForSingleObject(handle, 20_000);
        CloseHandle(handle);
    }
}

/// The application targets Windows 11 only. `GetVersionExW` lies without the
/// right manifest compatibility section, so ask ntdll directly.
fn ensure_windows_11() {
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    #[link(name = "ntdll")]
    extern "system" {
        fn RtlGetVersion(version_info: *mut OSVERSIONINFOW) -> i32;
    }

    let mut info: OSVERSIONINFOW = unsafe { std::mem::zeroed() };
    info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
    let status = unsafe { RtlGetVersion(&mut info) };
    if status != 0 || info.dwBuildNumber >= 22000 {
        return;
    }

    let message: Vec<u16> =
        "zapret-ui поддерживается только на Windows 11 (build 22000 и новее).\n\nОбновите Windows."
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
    let title: Vec<u16> = "zapret-ui"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
    std::process::exit(1);
}

/// Keeps the window reachable: if its rectangle does not intersect any monitor
/// (changed monitor layout, display-mode switch, stale position), center it on
/// the primary monitor.
fn ensure_on_screen(window: &tauri::WebviewWindow) {
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let Ok(monitors) = window.available_monitors() else {
        return;
    };

    let visible = monitors.iter().any(|monitor| {
        let origin = monitor.position();
        let extent = monitor.size();
        position.x < origin.x + extent.width as i32
            && position.x + size.width as i32 > origin.x
            && position.y < origin.y + extent.height as i32
            && position.y + size.height as i32 > origin.y
    });
    if visible {
        return;
    }

    if let Ok(Some(monitor)) = window.primary_monitor() {
        let origin = monitor.position();
        let extent = monitor.size();
        let x = origin.x + ((extent.width as i32 - size.width as i32) / 2).max(0);
        let y = origin.y + ((extent.height as i32 - size.height as i32) / 2).max(0);
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

/// Shows the main window and makes sure it is not parked outside every monitor.
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        ensure_on_screen(&window);
        let _ = window.show();
        apply_titlebar_theme(&window, config::load_config().theme != "light");
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // The updater launches the new exe with --wait-for-pid before this process
    // exits, so the single-instance plugin is not blocked by the old instance.
    if let Some(pid) = arg_value("--wait-for-pid").and_then(|value| value.parse().ok()) {
        wait_for_process_exit(pid);
    }
    updater::cleanup_backup();
    ensure_windows_11();
    ensure_elevated();
    let is_autostart = std::env::args().any(|a| a == "--autostart");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::set_strategy,
            commands::start,
            commands::stop,
            commands::get_config,
            commands::get_whats_new,
            commands::dismiss_whats_new,
            commands::set_game_filter,
            commands::set_autostart,
            commands::set_check_updates,
            commands::set_check_app_updates,
            commands::set_theme,
            commands::set_runtime_path,
            commands::set_flashscore_bypass,
            commands::set_bench_auto_apply,
            commands::check_update,
            commands::check_app_update,
            commands::apply_update,
            commands::apply_app_update,
            commands::open_external,
            commands::open_runtime_folder,
            commands::run_bench,
            commands::run_search,
            commands::cancel_bench,
            commands::quick_check,
            commands::apply_candidate,
            commands::delete_generated,
            commands::get_last_bench,
            commands::get_tgws_status,
            commands::start_tgws,
            commands::stop_tgws,
            commands::restart_tgws,
            commands::get_tgws_link,
            commands::regenerate_tgws_secret,
            commands::get_tgws_logs,
            commands::check_tgws_update,
            commands::install_tgws_update,
            commands::dismiss_tgws_update,
            commands::set_tgws_autostart,
            commands::set_tgws_host,
            commands::set_tgws_port,
            commands::set_tgws_secret,
            commands::open_tgws_log,
            commands::open_telegram_link,
            commands::copy_to_clipboard,
        ])
        .setup(move |app| {
            // Tray
            let show_i =
                tauri::menu::MenuItem::with_id(app, "show", "Показать", true, None::<&str>)?;
            let start_i =
                tauri::menu::MenuItem::with_id(app, "start", "Запустить", true, None::<&str>)?;
            let stop_i =
                tauri::menu::MenuItem::with_id(app, "stop", "Остановить", true, None::<&str>)?;
            let quit_i = tauri::menu::MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&show_i, &start_i, &stop_i, &quit_i])?;

            let _tray = tauri::tray::TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        show_main_window(app);
                    }
                    "start" => {
                        let state = app.state::<AppState>();
                        let _ = crate::commands::start(state);
                    }
                    "stop" => {
                        let state = app.state::<AppState>();
                        let _ = crate::commands::stop(state);
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        show_main_window(app);
                    }
                })
                .build(app)?;

            // Hide window on close (minimize to tray) instead of exiting
            if let Some(w) = app.get_webview_window("main") {
                let dark = config::load_config().theme != "light";
                apply_titlebar_theme(&w, dark);
                let window = w.clone();
                w.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                });
            }

            // If the previous session died in the middle of a strategy check, the
            // bypass service is stopped; restore it before doing anything else.
            commands::recover_pending(&app.state::<AppState>());

            // Telegram proxy: start it in the background when the user asked for
            // it and the binary is already downloaded. A missing binary is not
            // an error — the Telegram page offers to download it on demand.
            {
                let enabled = {
                    let state = app.state::<AppState>();
                    let cfg = lock_config(&state);
                    cfg.tgws.autostart
                };
                if enabled && crate::config::tgws_binary_path().exists() {
                    let handle = app.handle().clone();
                    std::thread::spawn(move || {
                        let _ = crate::tgws::start(&handle);
                    });
                }
            }

            // On autostart, start hidden
            if is_autostart {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
                // Start last strategy silently (if the service isn't already running).
                let state = app.state::<AppState>();
                let cfg = lock_config(&state);
                let last = cfg.last_strategy.clone();
                let autostart_enabled = cfg.autostart;
                drop(cfg);
                if autostart_enabled && !last.is_empty() && !crate::engine::Engine::is_running() {
                    let _ = crate::commands::start(state);
                }
            } else {
                // Normal launch: show the window on a visible monitor.
                show_main_window(app.handle());
            }

            // Self-heal the autostart Run key: if the user enabled autostart but the
            // value is missing or points to a different exe path, recreate it.
            {
                let cfg = app.state::<AppState>();
                let autostart_enabled = lock_config(&cfg).autostart;
                if autostart_enabled {
                    if let Ok(exe) = std::env::current_exe() {
                        let exe_str = exe.to_string_lossy().to_string();
                        let task_points_elsewhere = crate::autostart::registered_exe()
                            .map(|p| !p.eq_ignore_ascii_case(&exe_str))
                            .unwrap_or(true);
                        if !crate::autostart::is_installed() || task_points_elsewhere {
                            let _ = crate::autostart::install(&exe_str);
                        }
                    }
                }
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // The Telegram proxy is a child process: never leave it running
            // after the window is gone, otherwise port 1443 stays occupied.
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                crate::tgws::stop_on_exit(app_handle);
            }
        });
}

#[cfg(test)]
mod lock_tests {
    use super::lock_unpoisoned;
    use std::sync::{Arc, Mutex};

    #[test]
    fn recovers_a_poisoned_mutex() {
        let mutex = Arc::new(Mutex::new(42));
        let worker = mutex.clone();
        let _ = std::thread::spawn(move || {
            let _guard = worker.lock().unwrap();
            panic!("poison the lock");
        })
        .join();
        assert_eq!(*lock_unpoisoned(&mutex), 42);
    }
}
