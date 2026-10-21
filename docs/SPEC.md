# zapret-ui — Полная спека проекта и контекст

> **Назначение этого документа:** постановка задачи, все принятые решения и контекст
> разработки `zapret-ui`. Создан, чтобы следующая ИИ-сессия (терминал IntelliJ IDEA)
> могла продолжить работу **без уточняющих вопросов по уже принятым решениям**.
> Это одновременно спецификация и handoff-документ.
>
> Связанные артефакты:
> - Дизайн-спек (краткая, решения): `docs/superpowers/specs/2026-08-16-zapret-ui-design.md`
> - README: `README.md`
> - Скрипт сборки: `scripts/build.ps1`

---

## 1. Что это

**zapret-ui** — современный Windows-GUI-менеджер для утилиты обхода DPI
**zapret-discord-youtube** (проект Flowseal, форк `zapret`/GoodbyeDPI).
Заменяет ручной запуск `.bat`-стратегий и консольное меню `service.bat`
на красивое окно с треем.

- Репозиторий: `<repo>` (git, ветка `master`, уже создан коммит).
- Стек: **Tauri 2 (Rust)** + **Vue 3 + TypeScript + Vite**.
- Результат: портабельный **`zapret-ui.exe` ~12 МБ**, RAM ~40–60 МБ.

---

## 2. Постановка задачи (исходный запрос пользователя)

Пользователь попросил написать UI для zapret-discord-youtube с требованиями:

1. **Запуск программы при старте Windows.**
2. **Запоминание последней использованной стратегии** (пользователь использует `general (ALT12)`)
   и её автоматическое включение при старте.
3. **Проверка обновлений** (как сейчас в `service.bat`) **+ возможность обновления без ручного
   захода на GitHub** (скачивание релиза, удаление старой версии, добавление новой).
4. **Упаковать всё в один исполняемый `.exe`** (по аналогии с `tg-ws-proxy` от Flowseal),
   но с современным приятным UI (как Discord и современные приложения 2026).

Дополнительное пожелание из обсуждения:
- **UI должен быть устойчив к появлению новых стратегий** в обновлённых релизах
  (сейчас 22 стратегии, но если добавится больше — UI не должен ломаться; стратегии
  должны определяться динамически).
- **`winws.exe` не должен создавать «визуальный шум»** — никаких окон/иконок на панели задач.
- **Автозапуск без UAC-промпта при входе в Windows** (по образцу tg-ws-proxy: «запустилось и забыл»).

---

## 3. Ключевые решения (приняты в брейншторме — НЕ переспрашивать)

| Вопрос | Решение |
|---|---|
| Связь UI и runtime | UI управляет **отдельной папкой runtime** (`%LOCALAPPDATA%\zapret-ui\runtime`), не встроенной в exe |
| Форма приложения | **Окно + системный трей** (при старте Windows окно скрыто, живёт в трее) |
| Где хранить runtime/конфиг | **`%LOCALAPPDATA%\zapret-ui`** (не рядом с exe) |
| Охват обновлений | Обновляется **только runtime zapret** (не само UI). Новые стратегии определяются динамически |
| Права администратора | Приложение (release) при ручном запуске само перезапускается через UAC (`ensure_elevated`); при автозапуске — через Планировщик `/RL HIGHEST`, **без UAC** |
| Запуск обхода при старте | **Планировщик задач** (`schtasks /SC ONLOGON /RL HIGHEST`) + `winws.exe` как скрытый дочерний процесс (`CREATE_NO_WINDOW`) |
| Первый запуск | UI проверяет обновления; если runtime нет — кнопка «Check for updates / Download & Update» |
| Стек | **Tauri 2 (Rust) + Vue 3 + TS** (не Electron — 150–300 МБ RAM; не WPF — тяжёлый exe) |

---

## 4. Почему выбран стек (обоснование)

| Критерий | Electron | WPF/.NET | **Tauri 2 (выбран)** |
|---|---|---|---|
| Размер exe | 80–150 МБ | 60–150 МБ | **~12 МБ** |
| RAM (idle) | 150–300 МБ | 50–80 МБ | **~40–60 МБ** |
| Современный UI | да | сложнее | **да (Vue + CSS)** |
| Трей/автозапуск/single-instance | частично | частично | **официальные плагины** |

- `winws.exe` сам весит ~5 МБ и ест ~10–20 МБ — при ручном запуске работает только он.
  Electron добавил бы +150 МБ RAM, что нарушало ключевое требование. Tauri добавляет минимум.
- WebView2 уже установлен в Windows пользователя (v151) — браузер в exe не бандлим.
- Rust установлен во время сессии (rustup 1.29.0, тулчейн 1.97.1 x86_64-pc-windows-msvc).
  MSVC Build Tools 2022 уже были на машине.

---

## 5. Архитектура

```
┌────────────────────────────────────────────────┐
│  zapret-ui.exe (Tauri 2, Rust)                 │
│  ┌──────────────────────────────────────────┐  │
│  │ Rust backend (src-tauri/src/)            │  │
│  │  • strategy.rs  — парсер .bat → args     │  │
│  │  • engine.rs    — spawn/kill winws.exe   │  │
│  │  • probe.rs     — curl-проверки хостов   │  │
│  │  • bench.rs     — прогон и скоринг       │  │
│  │  • generator.rs — мутации стратегий      │  │
│  │  • sys.rs       — System32-утилиты       │  │
│  │  • runtime.rs   — GitHub API, zip, swap  │  │
│  │  • autostart.rs — Run-ключ               │  │
│  │  • config.rs    — config.json            │  │
│  │  • commands.rs  — Tauri-команды (IPC)    │  │
│  │  • lib.rs       — tray, single-instance  │  │
│  └──────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────┐  │
│  │ Vue 3 frontend (src/)                    │  │
│  │  • App.vue, style.css, types.ts          │  │
│  └──────────────────────────────────────────┘  │
└────────────────────────────────────────────────┘
        │ spawn (CREATE_NO_WINDOW — невидимый)
        ▼
  %LOCALAPPDATA%\zapret-ui\runtime\
  ├─ bin\winws.exe, WinDivert64.sys, *.bin
  ├─ lists\*.txt, *.bat (стратегии), service.bat
```

**Папки/файлы на диске:**

| Путь | Назначение |
|---|---|
| `%LOCALAPPDATA%\zapret-ui\config.json` | Конфиг: `last_strategy`, `autostart`, `check_updates`, `game_filter`, `flashscore_bypass`, `runtime_version`, `bench_budget`, `bench_auto_apply`, `apply_auto_rollback` |
| `%LOCALAPPDATA%\zapret-ui\runtime\` | Распакованный zapret (bin/, lists/, utils/, *.bat, service.bat) |
| `%LOCALAPPDATA%\zapret-ui\tmp\` | Временные файлы обновлений (`runtime_*`) |
| `%LOCALAPPDATA%\zapret-ui\bench.json` | История проверок (последние 20 прогонов) |
| `%LOCALAPPDATA%\zapret-ui\bench_state.json` | Маркер незавершённого прогона для восстановления после падения |
| `%LOCALAPPDATA%\zapret-ui\generated.json` | Экспериментальные стратегии (args, родитель, score) |

---

## 6. Реализация — подробности по модулям

### 6.1 `strategy.rs` — парсер стратегий (критичный, покрыт тестами)

- Сканирует `runtime\*.bat`, **пропускает `service*.bat`**.
- Из каждого `.bat` извлекает аргументы `winws.exe`:
  1. Склейка строк с `^` (перенос cmd-команды) — `join_continuations`.
  2. Поиск `winws.exe`, всё до него отбрасывается.
  3. Подстановка переменных: `%BIN%`, `%LISTS%`, `%GameFilterTCP%`, `%GameFilterUDP%`,
     `%~dp0`, `^!` → `!`, `^^` → `^`. **Важно:** `BIN`/`LISTS`/`%~dp0` подставляются с
     завершающим обратным слэшем (как в bat).
  4. Токенизация с учётом двойных кавычек (в отличие от cmd `for`, не режет по `=`,`;`).
- Natural sort стратегий (`general (ALT)` до `general (ALT2)`).
- **Тесты:** `strategy_tests.rs` — `parse_general_bat`, `parse_alt12_bat`, `discover_count`.
  ⚠️ Тесты захардкожены на путь `%LOCALAPPDATA%\zapret-ui\runtime`
  (машиноспецифично — известный «запах», см. раздел 9).

### 6.2 `engine.rs` — управление обходом как Windows-службой

Обход работает как **Windows-служба `zapret`** — ровно как `service.bat` «Install Service»:

- `Engine::install(exe, args, strategy)`:
  - очистка предыдущего состояния (см. `remove`);
  - `netsh interface tcp set global timestamps=enabled` (как `:tcp_enable`);
  - `sc create zapret binPath= "\"...\winws.exe\" <args>" DisplayName="zapret" start=auto`;
  - `sc description zapret "Zapret DPI bypass software"`;
  - `sc start zapret`;
  - `reg add HKLM\System\CurrentControlSet\Services\zapret /v zapret-discord-youtube ... /d <strategy>`
    (запоминает стратегию, как батник).
- `Engine::remove()` — как `service.bat` «Remove Services»: `net stop zapret` → `sc delete zapret`
  → `taskkill /IM winws.exe /F` → `net stop WinDivert` → `sc delete WinDivert` → `sc delete WinDivert14`.
  **Драйвер WinDivert всегда удаляется при остановке** — нет «утечки» драйвера в ядре.
- `Engine::is_running()` — `sc query zapret` (состояние RUNNING).
- `Engine::is_any_winws_running()` — `tasklist /FI "IMAGENAME eq winws.exe"` (fallback).

Служба `start=auto` (LocalSystem) сама поднимает обход при загрузке Windows, без участия UI.

### 6.3 `runtime.rs` — GitHub API, скачивание, обновление

- `LATEST_RELEASE_URL = https://api.github.com/repos/Flowseal/zapret-discord-youtube/releases/latest`
- `fetch_latest_release()` — reqwest (blocking), User-Agent `zapret-ui`.
- `installed_version()` — читает `runtime\service.bat`, строка `set "LOCAL_VERSION=..."`.
- `download_to_temp` + `extract_zip` (crate `zip`). В zip релиза всё лежит внутри
  одной папки `zapret-discord-youtube-<ver>/` — `extract_zip` «поднимает» её содержимое наверх.
- `is_runtime_valid()` — проверка `bin\winws.exe` существует.

### 6.4 `autostart.rs` — автозапуск через Run-ключ (видим в «Автозагрузке»)

- Автозапуск **приложения** — через **Run-ключ** `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`:
  значение `zapret-ui` = `"<exe>" --autostart`. Приложение видно в «Автозагрузке»
  Диспетчера задач и Настроек Windows (как TG WS Proxy).
- Включить: `reg add ... /v zapret-ui /t REG_SZ /d "\"<exe>\" --autostart" /f`.
- Выключить: `reg delete ... /v zapret-ui /f`.
- `registered_exe()` — читает путь из `reg query` (парсинг строки `REG_SZ`, покрыт тестами).
- **Self-heal:** при запуске приложения, если `autostart=true`, а значение отсутствует
  или путь ≠ текущему exe — пересоздаётся (Run-ключ «следует» за exe).
- **Миграция:** при включении/выключении удаляется старая задача Планировщика `zapret-ui`
  (устаревший механизм автозапуска).

Сам **обход** при этом стартует не приложением, а **службой `zapret`** (`start=auto`, см. 6.2).

### 6.5 `config.rs`

- `GameFilter` enum: `Disabled/All/Tcp/Udp` → значения `tcp_value()/udp_value()`:
  - Disabled: TCP=`12`, UDP=`12` (как в service.bat)
  - All: TCP=`1024-65535`, UDP=`1024-65535`
  - Tcp: TCP=`1024-65535`, UDP=`12`
  - Udp: TCP=`12`, UDP=`1024-65535`
- `load_config`/`save_config` в `%LOCALAPPDATA%\zapret-ui\config.json`.

### 6.6 `commands.rs` — IPC-команды (Rust ↔ Vue)

| Команда | Что делает |
|---|---|
| `get_state` | статус: версия приложения, стратегии, экспериментальные, current, running, версия runtime, runtime_valid |
| `set_strategy` | проверить и сохранить выбранную стратегию (+ рестарт если running) |
| `start` / `stop` | управление службой `zapret` |
| `get_config` | конфиг для UI |
| `set_game_filter` | смена game filter |
| `set_autostart` | включить/выключить автозапуск (Run-ключ) |
| `set_check_updates` | тумблер авто-проверки |
| `set_bench_auto_apply` | применять ли лучшую стратегию автоматически после проверки |
| `check_update` (async) | версия установленная/последняя, changelog, размер |
| `apply_update` (async) | скачать zip → проверить digest → распаковать → swap → сохранить user-листы → перезапустить стратегию; эмитит `update-progress` |
| `open_runtime_folder` | открыть папку runtime в Explorer |
| `quick_check` (async) | проверка текущей стратегии (или baseline без обхода) по списку целей |
| `run_bench` (async) | baseline → все стратегии → скоринг; `autoApply` управляет автоприменением; эмитит `bench-progress` |
| `cancel_bench` | отмена прогона между кандидатами |
| `run_search` (async) | мутации топ-3 → прогон экспериментальных → сохранение победителей |
| `apply_candidate` | применить стратегию + отложенная перепроверка через 60 с с автооткатом |
| `delete_generated` | удалить экспериментальную стратегию |
| `get_last_bench` | последний прогон (из памяти или `bench.json`) |

### 6.7 `lib.rs` — трей, single-instance, autostart-флаг

- `ensure_elevated()` — в release, если не админ: перезапуск через **`ShellExecuteExW` с verb `runas`** и выход (раньше — PowerShell `Start-Process -Verb RunAs`, что давало вспышку окна терминала и молча падало при политике UAC «повышать без запроса»). При сбое — нативный `MessageBoxW` с ошибкой.
- `--autostart` флаг: окно скрыто, при `cfg.autostart` тихо стартует последняя стратегия.
- Обычный запуск: окно показывается.
- `tauri-plugin-single-instance`: повторный запуск фокусирует существующее окно.
- Трей: пункты **Показать / Запустить / Остановить / Выход**; клик ЛКМ по иконке — показать окно.
- Закрытие окна (X) → **hide в трей**, не выход.

### 6.8 Frontend (`src/`)

- **Стор**: `src/store.ts` — единый composable-singleton (refs + actions), без Pinia.
  Компоненты не прокидывают пропсы через полдерева, а импортируют состояние и действия.
- **Компоненты** (`src/components/`): `Panel` (панель с номером шага), `StatusPanel`
  (шаг 1: статус, активная и выбранная стратегия, старт/стоп, онбординг runtime),
  `StrategyPanel` (шаг 2: проверка, подбор, сортировка, список), `StrategyRow`
  (строка с баллом, D/Y/C, задержкой и вердиктом), `MaintenancePanel` (шаг 3:
  обновления, настройки и версия приложения), `Toggle`, `ConfirmDialog`, `ToastHost`.
- **Заголовок окна — нативный Windows 11** (`decorations: true`, `theme: "Dark"`
  + `DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE)`): системные caption-кнопки,
  Snap Layouts, корректные DPI и ресайз. Кастомного titlebar нет.
- **Сервисы**: `lib/confirm.ts` — promise-API модальных подтверждений (включая чекбокс
  «применить лучшую автоматически»), `lib/toast.ts` — стек тостов с severity и
  автоскрытием, `lib/labels.ts` — подписи вердиктов/статусов и форматирование.
- **Раскладка**: две колонки (слева панели 1 и 3, справа широкая панель 2); при ширине
  < 980 px — одна колонка, стратегии первыми. Окно 1040×720, минимум 760×560.
- **Поведение**: клик по строке только выбирает стратегию; применение — отдельной кнопкой
  (при работающем обходе — с подтверждением). Остановка обхода, обновление runtime,
  удаление экспериментальной стратегии и полная проверка требуют подтверждения.
- **Доступность**: `:focus-visible` на всех интерактивных элементах, строки доступны
  с клавиатуры (Enter/Space), `aria-label` у иконочных кнопок, контраст `--text-3`
  поднят до ≥ 4.5:1.
- **Polling** статуса раз в 4 секунды, пауза при скрытом окне и во время прогона;
  слушает `update-progress`, `bench-progress`, `auto-rollback`, `bench-warning`.
- **Результаты проверки**: дата прогона, число хостов, легенда D/Y/C, кнопка «Повторить»;
  строки без замеров помечены «нет данных» и уходят вниз при сортировке по баллу.

### 6.9 Конфигурация Tauri

- `tauri.conf.json`: `decorations: true`, `theme: "Dark"`, `visible: false` (окно создаётся
  скрытым, показывается в `lib.rs` setup), размер 1040×720, минимум 760×560.
  Нативный заголовок Windows 11; тёмный caption через DWM. `csp` задан
  (CSP: `default-src 'self'`, `script-src 'self'`, `style-src 'self' 'unsafe-inline'`,
  `connect-src ipc: http://ipc.localhost`).
- `capabilities/default.json`: только `core:default`; кастомных window-операций из
  фронтенда нет (окно управляется нативно и из Rust-трея).

---

## 6.10 Проверка стратегий и экспериментальные стратегии

**Цель:** понять, какие стратегии реально работают для Discord/YouTube на текущем
провайдере, и подобрать новую, если рабочие перестали работать.

- **Цели** берутся из `runtime\utils\targets.txt` (4 Discord, 4 YouTube, 4 контрольных
  Google/Cloudflare). `PING:`-строки игнорируются: ICMP не показывает DPI-блокировку.
  Если файла нет — встроенный fallback-список.
- **Проверка** — `curl.exe` в трёх режимах (HTTP/1.1, TLS1.2, TLS1.3) с `--noproxy *`.
  Статусы: `ok`, `ssl` (подмена сертификата), `reset`, `timeout`, `dns`, `unsupported`,
  `error`. Ошибка DNS **не считается виной стратегии** и исключается из скоринга;
  `unsupported` — тоже нейтрален.
- **Скоринг:** Discord ×2, YouTube ×2, контрольные ×1. Вердикты: `works ≥ 90`,
  `partial 60–89`, `broken < 60`; провал контрольного хоста — `disqualified`
  (стратегия ломает посторонние сайты). Тай-брейк — средняя задержка.
- **Baseline-гейт:** перед прогоном замер без обхода. Если ≥80 % целевых хостов
  доступны — вердикт «DPI не блокирует», прогон не запускается.
- **Прогон** идёт через временные процессы `winws.exe` (не через пересоздание службы):
  `Engine::remove()` один раз → на каждого кандидата spawn → 600 мс прогрев → проверка
  → kill. После прогона служба поднимается с лучшей (или прежней) стратегией.
- **Отмена** — `AtomicBool`, проверяется между кандидатами; после отмены служба
  восстанавливается. **Падение приложения** посреди прогона лечится маркером
  `bench_state.json` при следующем старте (`commands::recover_pending`).
- **Генерация:** мутации 1–2 ключей (`--dpi-desync`, `split-pos`, `fooling`, `repeats`,
  `split-seqovl`, `ttl`, fake-файлы) у топ-3 стратегий. Меняются только значения уже
  присутствующих ключей; фильтры/порты/hostlists/`--wf-*` не трогаются. Валидация:
  существование файлов, структура `--new`, отсутствие `--wf-*` внутри профиля.
  Имена `auto-N` не пересекаются с официальными `.bat`. Бюджет по умолчанию 40 (20–100),
  прогон детерминирован при фиксированном seed.
- **Применение:** лучший кандидат ставится автоматически **только при включённой
  настройке** `bench_auto_apply` (чекбокс в диалоге проверки/подбора, по умолчанию выключен).
  Иначе показывается результат и кнопка «Применить» в строке. После применения через 60 с
  watchdog перепроверяет стратегию и откатывает на прежнюю, если та `broken`/`disqualified`
  (при `apply_auto_rollback = true`). Ошибки применения/отката показываются в UI.
- **Целостность:** системные утилиты вызываются по абсолютным путям `%SystemRoot%\System32`;
  записи zip проверяются через `enclosed_name` (защита от zip-slip); если GitHub публикует
  `digest` ассета, архив проверяется по SHA-256 до распаковки; конфиг пишется атомарно.
- **Ограничения:** живут только на Windows (нужен `curl.exe` и WinDivert); проверка
  TCP/TLS не показывает голосовой Discord (UDP) и QUIC; прогон прерывает обход
  на 2–5 минут; результаты — снимок на текущий момент и провайдера.

---

## 7. Как собрать / запустить / тестировать

```powershell
# Зависимости (Node 20.20.2 — см. раздел 8)
nvm use 20.20.2
cd <repo>
npm install

# Dev-режим (hot-reload)
npm run tauri dev

# Только фронтенд
npm run build

# Тесты Rust-бэкенда (парсер стратегий)
cd src-tauri
cargo test

# Release-сборка (портабельный exe + NSIS/MSI)
cd ..
.\scripts\build.ps1        # или: $env:CARGO_TARGET_DIR=".\src-tauri\target-dist"; npm run tauri build
```

**Артефакты сборки:**
- `zapret-ui.exe` (**в корне проекта**) — готовый портабельный exe (~12 МБ), копируется
  `scripts/build.ps1` из `src-tauri\target-dist\release\` после сборки.
- `src-tauri\target-dist\release\zapret-ui.exe` — внутренний exe.
- `src-tauri\target-dist\release\bundle\nsis\zapret-ui_1.0.0_x64-setup.exe` — установщик.
- `src-tauri\target-dist\release\bundle\msi\zapret-ui_1.0.0_x64_en-US.msi` — MSI.

> Почему `target-dist`: обычный `target\release\zapret-ui.exe` может быть заблокирован
> работающим elevated-процессом (Windows не даёт перезаписать exe). `scripts/build.ps1`
> собирает в отдельную папку через `CARGO_TARGET_DIR`.

**Проверка работы:**
- `cargo test` → 47 unit-тестов (парсер, probe/скоринг, генератор, zip-slip/digest,
  sys-пути, runtime, autostart) + 2 ручных теста с `#[ignore]`.
- Живые проверки (нужен админ; на время бенча служба останавливается, в конце
  восстанавливается автоматически):
  ```powershell
  cargo test live_probe_smoke -- --ignored --nocapture
  cargo test live_bench -- --ignored --nocapture
  ```
- Debug-запуск (`target\debug\zapret-ui.exe`) не требует UAC → удобен для проверки UI.
- Release-запуск перезапустится через UAC (требуются права админа для WinDivert).
- Для полного E2E-теста (старт стратегии через UI) нужен админ-терминал или release-запуск.

---

## 8. Окружение разработки

- **ОС:** Windows 11 x64, локаль RU.
- **Node.js:** 20.x (Vite, TypeScript, Tauri CLI).
- **Rust:** stable-x86_64-pc-windows-msvc, MSVC Build Tools 2022.
- **WebView2 Runtime:** входит в Windows 11, отдельно не бандлится.
- **Оригинальный zapret:** эталонная распаковка релиза zapret-discord-youtube;
  для тестов используется `%LOCALAPPDATA%\zapret-ui\runtime`.
- **Референс:** [Flowseal/tg-ws-proxy](https://github.com/Flowseal/tg-ws-proxy) —
  пример «один exe, живёт в трее».

---

## 9. Текущее состояние и известные проблемы (TODO)

**Статус:** v1.0.1 — все баги запуска/автозапуска исправлены и проверены E2E, UI переведён
на русский, добавлены DY-иконки и палитра Discord+YouTube, тесты переведены на фикстуры.

**Что исправлено в v1.0.1:**

1. **Элевация (баг 2):** `ensure_elevated` переписан на `ShellExecuteExW(runas)` — ушла
   вспышка PowerShell-окна, приложение надёжно запускается из двойного клика
   (проверено: elevated-инстанс стартует при политике UAC «повышать без запроса»).
2. **Автозапуск (баг 1):** исправлен квотинг `/TR` (внешние кавычки) + self-heal задачи
   при каждом запуске, если `autostart=true`, а задачи нет. Проверено E2E: задача
   `zapret-ui` создаётся с корректным XML (`LogonTrigger`, `RunLevel=HighestAvailable`,
   `Command`+`Arguments`).
3. **Перетаскивание окна (баг 3):** добавлен permission `core:window:allow-start-dragging`.
4. **UI:** убран поиск стратегий; полная русская локализация (UI, трей, ошибки/прогресс);
   тост «Доступно обновление vX»; палитра Discord blurple `#5865F2` + YouTube red `#FF0000`;
   логотип-градиент.
5. **Иконки:** сгенерированы иконки «DY» (градиентный фон blurple→red + белые буквы,
   как логотип в окне) для окна, трея и установщиков (`scripts/gen-icons.ps1` +
   `npm run tauri icon`).
6. **Тесты:** `strategy_tests.rs` переведён на фикстуры во временной папке (нет
   машинной зависимости); добавлены тесты `installed_version`, `find_zip_asset`,
   `is_update_available`, `task_action`; исправлен `natural_cmp` (посимвольный natural
   sort — `general (ALT)` корректно идёт перед `general (ALT2)`).

**Что сделано в v1.0.2 (переход на принцип службы, 1-в-1 с `service.bat`):**

1. **Обход — Windows-служба `zapret`** (как «Install Service»): winws стартует как служба
   (LocalSystem, `start=auto`) и поднимается сама при загрузке Windows.
2. **«Остановить обход» чистит систему** (как «Remove Services»): останавливает/удаляет службу,
   убивает `winws.exe` и **удаляет драйвер WinDivert** — устранена «утечка» драйвера в ядре.
3. **Автозапуск приложения** — через **Run-ключ** (видно в «Автозагрузке» Диспетчера задач);
   старая задача Планировщика удаляется (миграция).
4. **UAC отключён** (`EnableLUA=0`, потребуется перезагрузка).

**Что сделано в v1.1.0 (проверка стратегий и автоподбор):**
1. **Проверка стратегий** (`probe.rs`, `bench.rs`): цели из `utils\targets.txt`,
   `curl.exe` (HTTP/1.1 + TLS1.2 + TLS1.3), baseline-гейт, скоринг Discord×2/YouTube×2/
   контроль×1, отмена, история в `bench.json`, восстановление после падения.
2. **Экспериментальные стратегии** (`generator.rs`): мутации топ-3, валидация путей и
   `--new`, дедуп, `generated.json`, группа «Экспериментальные» в UI.
3. **Применение с откатом**: лучший кандидат ставится автоматически только при включённой
   настройке (чекбокс в диалоге, по умолчанию выключен); через 60 с watchdog перепроверяет
   стратегию и при `broken`/`disqualified` возвращает прежнюю.
4. **UI v1.1.0**: панели с шагами (1 Подключение, 2 Стратегии, 3 Обслуживание), компоненты
   и composable-store, диалоги подтверждения, стек тостов, онбординг runtime, сортировка
   по баллу/задержке/имени, метаданные прогона (дата, число хостов, легенда), доступность
   с клавиатуры и focus-visible, адаптивная раскладка.
5. **Безопасность**: защита от zip-slip, проверка `digest` ассета, абсолютные пути
   System32-утилит, атомарная запись `config.json`, CSP, удаление мёртвых IPC-команд
   и неиспользуемого `opener`-плагина, пере-валидация `generated.json` при применении.
6. **Надёжность**: восстановление обхода при ошибке обновления/бенча, явные ошибки
   восстановления, подтверждения остановки/обновления/проверки/удаления, проверка
   окна на попадание в видимую область монитора.

**Известные проблемы / замечания:**

1. ~~Первый запуск: авто-скачивание runtime не доведено до «одного клика».~~ Исправлено
   в v1.1.0: панель «Подключение» предлагает «Скачать и установить».
2. **Пользовательские user-листы** (`*-user.txt`, `ipset-exclude-user.txt`) сохраняются при
   обновлении, но **бэкап `ipset-all.txt.backup` и `utils\*` не переносятся**.
3. **UI-обновление самого себя** не реализовано (обновляется только runtime) — намеренно.
4. ~~В `apply_update` при сбое стратегия не перезапустится.~~ Исправлено в v1.1.0:
   ошибка обновления восстанавливает прежнюю стратегию.
5. **Проверка «вход в Windows»:** задача создана и корректна; полный цикл (лог-он →
   авто-запуск → скрытое окно → старт стратегии) проверяется при следующем входе в систему
   или через `schtasks /Run` (запустит обход: убьёт текущий ручной `winws.exe`).
6. `cmd.exe`/PowerShell в окружении агента — **HIGH integrity, но урезанный токен**:
   `schtasks /Create` с `/RL HIGHEST`/`/SC ONLOGON` из него даёт «Отказано в доступе».
   Это артефакт токена агента, НЕ приложения: elevated-инстанс (через `runas`) создаёт задачу
   без проблем (подтверждено E2E).
7. **Проверка стратегий — одиночный замер:** каждый хост проверяется по одному разу на
   протокол, поэтому нестабильный канал может сдвинуть score на ±10. Повторные прогоны
   надёжнее; история лежит в `bench.json`.

---

## 10. Что уже решено — НЕ переспрашивать

- Стек: **Tauri 2 + Vue 3 + TS** (окончательно).
- Runtime в **`%LOCALAPPDATA%\zapret-ui\runtime`**, не рядом с exe.
- Обновляется **только runtime**, само UI — нет.
- Автозапуск приложения — **Run-ключ HKCU**; старая задача Планировщика удаляется (миграция).
- Обход — **Windows-служба `zapret`** (`start=auto`, LocalSystem) — принцип `service.bat` «Install Service».
- Остановка обхода = «Remove Services»: служба + winws + **драйвер WinDivert удаляются**.
- **UAC отключён** (`EnableLUA=0`) на текущей машине; на обычной машине автозапуск может
  показать запрос UAC.
- UI: **окно + трей**; закрытие окна → в трей.
- Элевация — **`ShellExecuteExW(runas)`** (не PowerShell, не манифест `requireAdministrator`).
- Список стратегий — **динамический** (новые .bat появляются без пересборки).
- Язык интерфейса — **русский**; палитра — Discord blurple + YouTube red.
- Иконки — как логотип в окне: скруглённый квадрат с градиентом blurple→red и белыми
  буквами «DY» (генератор: `scripts/gen-icons.ps1`, затем `npm run tauri icon`).
  ⚠️ После смены иконок обязательна **полная чистая пересборка** (удалить
  `src-tauri\target-dist`), иначе cargo переиспользует закэшированный ресурс `.ico`.
- Проверка стратегий — **только вручную** (никаких фоновых переборов), цели из
  `utils\targets.txt`, ICMP не используется.
- Бенч запускает кандидатов **временными процессами `winws.exe`**, служба пересоздаётся
  один раз в конце.
- Автоприменение победителя — **opt-in** (`bench_auto_apply`, по умолчанию выключено;
  чекбокс в диалоге проверки/подбора).
- UI — **панели с шагами** (1 Подключение, 2 Стратегии, 3 Обслуживание), компоненты
  и composable-store, без UI-фреймворка (только Vue + CSS-токены).
- Окно — **1040×720**, минимум 760×560; при старте проверяется попадание в видимую
  область монитора.
- Безопасность — **CSP**, абсолютные пути `%SystemRoot%\System32`, атомарная запись
  конфига, проверка `digest` ассета, защита от zip-slip.
- Экспериментальные стратегии — **`generated.json`**, отдельная группа в UI, имена `auto-N`
  не пересекаются с официальными `.bat`.
- Лучший кандидат применяется автоматически, через 60 с — перепроверка и автооткат.

---

## 11. Рекомендуемые скиллы для следующей сессии

Следующему агенту (в IntelliJ-терминале) рекомендуется вызвать:
- **brainstorming** — перед любой новой фичей/изменением поведения.
- **writing-plans / executing-plans** — для многошаговых задач (например, пункты 9.3/9.4).
- **test-driven-development** — при доработке парсера стратегий/обновлений.
- **systematic-debugging** — при багах (winws не стартует, обновление не применяется и т.п.).
- **verification-before-completion** — перед утверждением «готово» (запустить `cargo test`,
  `npm run build`, проверить сборку).
- **requesting-code-review** — перед мержем/релизом.
- **update-skills / handoff** — при длинных сессиях или передаче контекста.
- **using-git-worktrees** — для изолированной работы над фичами (по желанию).

---

## 12. Шпаргалка команд (быстрый старт для агента)

```powershell
nvm use 20.20.2
cd <repo>
$env:Path = [Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [Environment]::GetEnvironmentVariable("Path","User")

npm run tauri dev          # dev
cargo test                 # тесты (cd src-tauri)
cargo test live_bench -- --ignored --nocapture   # живой E2E (админ)
.\scripts\build.ps1        # release
# результат: src-tauri\target-dist\release\zapret-ui.exe
```
