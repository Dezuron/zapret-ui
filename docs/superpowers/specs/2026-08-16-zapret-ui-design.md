# zapret-ui — Design Spec

## Purpose

Modern Windows GUI manager for `zapret-discord-youtube` (Flowseal) DPI bypass runtime.
Replaces manual `.bat` strategy launches and `service.bat` menu with a polished app that:

1. Runs at Windows startup without a UAC prompt.
2. Remembers the last used strategy (e.g. `general (ALT12)`) and auto-enables it at boot.
3. Checks GitHub for new runtime releases and can update automatically (download zip, replace files) after user confirmation.
4. Is packaged as a single portable `.exe`.
5. Hides `winws.exe` completely — only the app icon is visible (tray + window).

## Stack

- **Tauri 2** (Rust backend, ~10 MB exe, ~40–60 MB RAM idle; WebView2 shared with Edge).
- **Vue 3 + TypeScript + Vite** frontend (modern desktop app look).
- **electron-builder-free**: `tauri` bundling → portable exe (NSIS-less, single-file target).

Chosen over Electron (150–300 MB RAM) and WPF (heavier exe) because RAM/exe-size are top
requirements; WebView2 is already installed on the machine (v151).

## Runtime model

- App owns a private runtime folder: `%LOCALAPPDATA%\zapret-ui\runtime\`.
  - Contains the extracted release: `bin\`, `lists\`, `utils\`, `*.bat`, `service.bat`.
- First run: app auto-downloads latest release zip from GitHub (`/releases/latest`,
  asset `zapret-discord-youtube-<ver>.zip`), extracts it into runtime.
- Updates: fetch latest release via GitHub API → compare tag vs installed version →
  ask user (with changelog) → download zip to temp → extract to `runtime.new` →
  swap directories atomically → restart current strategy.
- User lists (`*-user.txt`, `ipset-exclude-user.txt`) are preserved across updates.
- Config stored in `%LOCALAPPDATA%\zapret-ui\config.json`:
  `{ last_strategy, autostart, check_updates, runtime_version, game_filter }`.

## Strategy handling

- Scan `runtime\*.bat` (skip `service*.bat`).
- Parse each `.bat` to extract the `winws.exe` command-line (logic ported from
  `service.bat` parser): find `winws.exe`, capture args after it across `^` line
  continuations, substitute `%BIN%`/`%LISTS%`/`%GameFilterTCP%`/`%GameFilterUDP%`/
  `%~dp0`.
- Start strategy: spawn `runtime\bin\winws.exe` as hidden child (`CREATE_NO_WINDOW`),
  no console, no taskbar icon.
- Stop: kill the child process (track PID) + `taskkill` fallback by image name.
- UI list is dynamic — new strategies from updated releases appear automatically.

## Autostart (no UAC at boot)

- **Scheduled Task** (`schtasks /Create /SC ONLOGON /RL HIGHEST /F`) created once by the
  app (app runs elevated). Task launches the app exe with `--autostart` flag.
- At boot, Windows runs the app elevated silently (scheduled task = no UAC prompt).
- App with `--autostart` flag: auto-starts last strategy if autostart enabled, stays in tray.
- App is a single-instance app (mutex / tauri-plugin-single-instance); second launch
  focuses the existing window.

## Admin rights

- App requires admin to run `winws.exe` (WinDivert driver). On first normal launch, it
  relaunches itself elevated via UAC. The scheduled task path avoids
  UAC at boot.
- `winws.exe` loads `WinDivert64.sys` from its own `bin\` automatically — no separate
  driver install needed.

## UI (Vue 3)

Modern dark theme (Discord style):
- **Header**: app title, current status pill (Running/Stopped/Error), version.
- **Main panel**: big Start/Stop toggle button; strategy list (radio cards, current
  strategy highlighted, search box).
- **Settings**: autostart toggle, update-check toggle, game filter mode (Off/TCP/UDP/Both),
  "open runtime folder", "check for updates".
- **Update card**: shows installed version + latest version + changelog + "Update" button.
- Tray icon: menu (Show, Start/Stop, Exit); close window → hide to tray (not exit).

## Files

- `src-tauri/src/lib.rs` — app setup, tray, single-instance, autostart flag.
- `src-tauri/src/commands.rs` — Tauri commands (strategies, start/stop, update, config).
- `src-tauri/src/strategy.rs` — `.bat` parser + strategy discovery.
- `src-tauri/src/engine.rs` — winws spawn/kill/status.
- `src-tauri/src/runtime.rs` — runtime dir, download, extract, atomic update, version.
- `src-tauri/src/autostart.rs` — scheduled task create/remove/query.
- `src-tauri/src/config.rs` — config load/save.
- `src/frontend/...` — Vue 3 app.

## Non-goals (v1)

- Updating the UI app itself (only runtime update). Could add later.
- Full port of every `service.bat` menu feature (diagnostics, ipset, cache clearing).

## Success criteria

- Single portable exe builds and runs on Windows 10/11.
- Boot autostart without UAC; last strategy starts automatically.
- Update flow: check → changelog → confirm → download → swap → restart strategy.
- `winws.exe` never shows a window/taskbar icon.
