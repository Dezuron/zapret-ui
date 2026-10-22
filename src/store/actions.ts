import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  AppStatus,
  AppUpdateInfo,
  BenchProgress,
  BenchSummary,
  ConfigView,
  QuickCheck,
  TgwsStatus,
  TgwsUpdateInfo,
  UpdateInfo,
  WhatsNew,
} from "../types";
import { confirmAction } from "../lib/confirm";
import { toast } from "../lib/toast";
import { tgwsLogTail } from "../lib/tgws";
import {
  status,
  config,
  updateInfo,
  checkingUpdate,
  updateProgress,
  appUpdate,
  checkingAppUpdate,
  appUpdateProgress,
  wizardDismissed,
  whatsNew,
  busy,
  selected,
  starting,
  benchSummary,
  benchProgress,
  benchRunning,
  benchBusy,
  quick,
  budget,
  running,
  currentName,
  tgwsStatus,
  tgwsUpdate,
  tgwsLogs,
  tgwsLink,
  tgwsBusy,
  tgwsChecking,
  tgwsProgress,
} from "./state";

// ---------- polling ----------

let pollTimer: number | null = null;
let appUpdateTimer: number | null = null;
let lastRefreshError = 0;
let refreshSeq = 0;
let unlisteners: Array<() => void> = [];
let initialized = false;
let budgetInitialized = false;

export function applyTheme(theme: string | undefined) {
  const normalized = theme === "light" || theme === "gray" ? theme : "dark";
  document.documentElement.dataset.theme = normalized;
  document.documentElement.classList.toggle("dark", normalized !== "light");
}

async function refreshInternal(quiet: boolean) {
  // Allow overlapping calls (a polling tick may coincide with an action) but
  // only let the latest one write state, so responses cannot land out of order.
  const seq = ++refreshSeq;
  try {
    const nextStatus = await invoke<AppStatus>("get_state");
    if (seq !== refreshSeq) return;
    const nextConfig = await invoke<ConfigView>("get_config");
    if (seq !== refreshSeq) return;
    status.value = nextStatus;
    config.value = nextConfig;
    applyTheme(nextConfig.theme);
    if (!budgetInitialized && config.value) {
      budget.value = config.value.bench_budget;
      budgetInitialized = true;
    }
    const exists =
      nextStatus.strategies.some((s) => s.name === selected.value) ||
      nextStatus.generated.some((g) => g.name === selected.value);
    if (!exists) selected.value = nextStatus.current_strategy || "";
  } catch (error) {
    const now = Date.now();
    if (!quiet || now - lastRefreshError > 30_000) {
      lastRefreshError = now;
      toast("error", `Не удалось обновить состояние: ${error}`);
    }
  }
}

export function refresh(quiet = true) {
  return refreshInternal(quiet);
}

function startPolling() {
  if (pollTimer !== null) return;
  pollTimer = window.setInterval(() => {
    // Do not spawn sc/tasklist while hidden in the tray or during a benchmark.
    if (!document.hidden && !benchRunning.value) {
      void refreshInternal(true);
    }
  }, 4000);
}

function stopPolling() {
  if (pollTimer !== null) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

function onVisibilityChange() {
  if (document.hidden) {
    stopPolling();
  } else {
    void refreshInternal(true);
    startPolling();
  }
}

export async function initStore() {
  if (initialized) return;
  initialized = true;

  unlisteners = [
    await listen<string>("update-progress", (event) => {
      updateProgress.value = event.payload;
    }),
    await listen<BenchProgress>("bench-progress", (event) => {
      // `benchRunning` is owned by runBenchCommand's lifecycle; progress events
      // must not re-arm it after the command has already finished.
      benchProgress.value = event.payload;
    }),
    await listen<string>("auto-rollback", async (event) => {
      toast("warning", event.payload);
      await refreshInternal(true);
    }),
    await listen<string>("bench-warning", (event) => {
      toast("warning", event.payload);
    }),
    await listen<string>("app-update-progress", (event) => {
      appUpdateProgress.value = event.payload;
    }),
    await listen<TgwsStatus>("tgws-status", (event) => {
      tgwsStatus.value = event.payload;
    }),
    await listen<string>("tgws-log", (event) => {
      tgwsLogs.value = tgwsLogTail([...tgwsLogs.value, event.payload]);
    }),
  ];

  await refreshInternal(false);
  void refreshTgws();
  await loadWhatsNew();
  if (config.value?.theme) {
    void invoke("set_theme", { theme: config.value.theme }).catch(() => undefined);
  }
  try {
    benchSummary.value = await invoke<BenchSummary | null>("get_last_bench");
  } catch (error) {
    console.warn("Не удалось загрузить историю проверок", error);
  }
  if (config.value?.check_updates) {
    void checkUpdate();
  }
  if (config.value?.check_app_updates) {
    appUpdateTimer = window.setTimeout(() => {
      void checkAppUpdate(true);
    }, 3000);
  }
  startPolling();
  document.addEventListener("visibilitychange", onVisibilityChange);
}

export function disposeStore() {
  stopPolling();
  if (appUpdateTimer !== null) {
    clearTimeout(appUpdateTimer);
    appUpdateTimer = null;
  }
  for (const unlisten of unlisteners) unlisten();
  unlisteners = [];
  document.removeEventListener("visibilitychange", onVisibilityChange);
  initialized = false;
}

// ---------- actions ----------

export async function toggleStart() {
  if (busy.value || benchRunning.value) return;
  const wasRunning = running.value;

  if (wasRunning) {
    const { ok } = await confirmAction({
      title: "Остановить обход?",
      text: "Служба zapret и драйвер WinDivert будут удалены. Discord и YouTube снова могут стать недоступны.",
      confirmLabel: "Остановить",
      danger: true,
    });
    if (!ok) return;
  }

  busy.value = true;
  starting.value = true;
  try {
    if (wasRunning) {
      await invoke("stop");
    } else {
      if (selected.value) {
        await invoke("set_strategy", { name: selected.value });
      }
      await invoke("start");
    }
    await refreshInternal(true);
    toast("success", wasRunning ? "Обход остановлен." : "Обход запущен.");
  } catch (error) {
    toast("error", String(error));
  } finally {
    busy.value = false;
    starting.value = false;
  }
}

export function selectStrategy(name: string) {
  if (benchRunning.value) return;
  selected.value = name;
}

export async function applyStrategy(name = selected.value) {
  if (!name || busy.value || benchRunning.value) return;
  if (name === currentName.value && running.value) {
    toast("info", "Эта стратегия уже активна.");
    return;
  }

  if (!running.value) {
    busy.value = true;
    try {
      await invoke("set_strategy", { name });
      await refreshInternal(true);
      toast("info", `Выбрана «${name}».`);
    } catch (error) {
      toast("error", String(error));
    } finally {
      busy.value = false;
    }
    return;
  }

  const { ok } = await confirmAction({
    title: "Перезапустить обход?",
    text: `Текущее соединение прервётся, обход перезапустится со стратегией «${name}».`,
    confirmLabel: "Перезапустить",
  });
  if (!ok) return;

  busy.value = true;
  try {
    await invoke("apply_candidate", { name });
    await refreshInternal(true);
    toast("success", `Применена «${name}».`);
  } catch (error) {
    toast("error", String(error));
  } finally {
    busy.value = false;
  }
}

export async function quickCheck() {
  if (benchBusy.value || benchRunning.value) return;
  benchBusy.value = true;
  quick.value = null;
  try {
    quick.value = await invoke<QuickCheck>("quick_check");
  } catch (error) {
    toast("error", String(error));
  } finally {
    benchBusy.value = false;
  }
}

async function runBenchCommand(
  command: "run_bench" | "run_search",
  args: Record<string, unknown>,
  doneLabel: string,
) {
  if (benchBusy.value) return;
  benchBusy.value = true;
  benchRunning.value = true;
  quick.value = null;
  try {
    const summary = await invoke<BenchSummary>(command, args);
    benchSummary.value = summary;
    if (summary.applied) {
      toast("success", `${doneLabel} Применена «${summary.applied}».`);
    } else if (summary.message) {
      toast("warning", summary.message);
    } else if (summary.cancelled) {
      toast("info", "Проверка отменена, результаты частичные.");
    }
    await refreshInternal(true);
  } catch (error) {
    toast("error", String(error));
  } finally {
    benchBusy.value = false;
    benchRunning.value = false;
    benchProgress.value = null;
  }
}

export async function runBench(include: boolean) {
  if (benchBusy.value) return;
  const { ok, checked } = await confirmAction({
    title: "Проверить все стратегии?",
    text: "Обход будет остановлен на 2–5 минут, интернет станет нестабильным. После проверки служба поднимется снова.",
    confirmLabel: "Начать проверку",
    checkboxLabel: "Применить лучшую стратегию автоматически",
    checkboxValue: config.value?.bench_auto_apply ?? false,
  });
  if (!ok) return;
  await runBenchCommand(
    "run_bench",
    { includeGenerated: include, autoApply: checked },
    "Проверка завершена.",
  );
}

export async function runSearch() {
  if (benchBusy.value) return;
  const { ok, checked } = await confirmAction({
    title: "Подобрать новые стратегии?",
    text: "Сначала проверятся существующие, затем 20–100 мутаций. Это займёт 5–10 минут и прервёт обход: интернет будет нестабильным.",
    confirmLabel: "Начать подбор",
    checkboxLabel: "Применить лучшую стратегию автоматически",
    checkboxValue: config.value?.bench_auto_apply ?? false,
  });
  if (!ok) return;
  await runBenchCommand(
    "run_search",
    { budget: budget.value, autoApply: checked },
    "Подбор завершён.",
  );
}

export async function cancelBench() {
  try {
    await invoke("cancel_bench");
  } catch (error) {
    toast("error", String(error));
  }
}

export async function deleteGenerated(name: string) {
  if (benchRunning.value) return;
  const { ok } = await confirmAction({
    title: "Удалить экспериментальную стратегию?",
    text: `«${name}» будет удалена. Это действие нельзя отменить.`,
    confirmLabel: "Удалить",
    danger: true,
  });
  if (!ok) return;
  try {
    await invoke("delete_generated", { name });
    await refreshInternal(true);
    toast("success", `«${name}» удалена.`);
  } catch (error) {
    toast("error", String(error));
  }
}

export async function checkUpdate(silent = true) {
  if (checkingUpdate.value) return;
  checkingUpdate.value = true;
  try {
    updateInfo.value = await invoke<UpdateInfo>("check_update");
    if (updateInfo.value.update_available) {
      toast("info", `Доступно обновление flowseal-zapret ${updateInfo.value.latest}.`);
    } else if (!silent) {
      toast("info", "Обновлений flowseal-zapret нет.");
    }
  } catch (error) {
    if (!silent) {
      toast("error", String(error));
    }
  } finally {
    checkingUpdate.value = false;
  }
}

export async function checkAppUpdate(silent = true) {
  if (checkingAppUpdate.value) return;
  checkingAppUpdate.value = true;
  try {
    const info = await invoke<AppUpdateInfo | null>("check_app_update");
    appUpdate.value = info;
    if (info?.available) {
      toast("info", `Доступно обновление приложения ${info.latest}.`);
    } else if (!silent) {
      toast("info", "Обновлений приложения нет.");
    }
  } catch (error) {
    // A private repository or a missing release is reported as "no update";
    // a silent startup check must not spam the user with network errors.
    if (!silent) {
      toast("error", String(error));
    }
  } finally {
    checkingAppUpdate.value = false;
  }
}

export async function applyAppUpdate() {
  if (!appUpdate.value?.available || busy.value) return;
  const info = appUpdate.value;
  const { ok } = await confirmAction({
    title: "Обновить приложение?",
    text: `Приложение обновится до ${info.latest} и перезапустится. Текущая версия — ${info.current}.`,
    confirmLabel: "Обновить",
  });
  if (!ok) return;

  busy.value = true;
  appUpdateProgress.value = "Подготовка обновления…";
  try {
    await invoke("apply_app_update");
    // On success the backend exits this process; the new exe starts itself.
  } catch (error) {
    toast("error", String(error));
    busy.value = false;
    appUpdateProgress.value = null;
  }
}

export async function setTheme(theme: "dark" | "gray" | "light") {
  applyTheme(theme);
  try {
    await invoke("set_theme", { theme });
    await refreshInternal(true);
  } catch (error) {
    toast("error", String(error));
  }
}

export async function setCheckAppUpdates(enabled: boolean) {
  try {
    await invoke("set_check_app_updates", { enabled });
    await refreshInternal(true);
  } catch (error) {
    toast("error", String(error));
  }
}

export async function openReleasePage(url: string) {
  try {
    await invoke("open_external", { url });
  } catch (error) {
    toast("error", String(error));
  }
}

export async function applyUpdate() {
  if (busy.value || benchRunning.value) return;
  const { ok } = await confirmAction({
    title: "Обновить flowseal-zapret?",
    text: "Обход будет остановлен на время обновления и запущен снова. Пользовательские списки сохранятся.",
    confirmLabel: "Обновить",
  });
  if (!ok) return;

  busy.value = true;
  try {
    await invoke("apply_update");
    updateInfo.value = null;
    await refreshInternal(true);
    toast("success", "flowseal-zapret обновлён.");
  } catch (error) {
    toast("error", String(error));
  } finally {
    busy.value = false;
    updateProgress.value = null;
  }
}

export async function installRuntime() {
  if (busy.value) return;
  busy.value = true;
  try {
    const info = await invoke<UpdateInfo>("check_update");
    updateInfo.value = info;
    if (!info.update_available) {
      toast("warning", "Не удалось найти обновление flowseal-zapret.");
      return;
    }
    await invoke("apply_update");
    await refreshInternal(true);
    toast("success", "flowseal-zapret установлен.");
  } catch (error) {
    toast("error", String(error));
  } finally {
    busy.value = false;
    updateProgress.value = null;
  }
}

export function dismissSetupWizard() {
  wizardDismissed.value = true;
}

async function loadWhatsNew() {
  try {
    whatsNew.value = await invoke<WhatsNew | null>("get_whats_new");
  } catch (error) {
    console.warn("Не удалось загрузить «Что нового»", error);
  }
}

export async function dismissWhatsNew() {
  if (!whatsNew.value) return;
  try {
    await invoke("dismiss_whats_new");
  } catch (error) {
    toast("error", String(error));
  }
  whatsNew.value = null;
}

export async function chooseRuntimeFolder(): Promise<boolean> {
  if (busy.value || benchRunning.value) {
    toast("warning", "Дождитесь завершения текущей операции.");
    return false;
  }

  if (running.value) {
    const { ok } = await confirmAction({
      title: "Изменить папку flowseal-zapret?",
      text: "Для смены папки обход будет остановлен. После изменения запустите его снова вручную.",
      confirmLabel: "Остановить и изменить",
    });
    if (!ok) return false;
    try {
      await invoke("stop");
      await refreshInternal(true);
    } catch (error) {
      toast("error", String(error));
      return false;
    }
  }

  try {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Выберите папку для flowseal-zapret",
    });
    if (typeof selected !== "string") return false;
    const resolved = await invoke<string>("set_runtime_path", { path: selected });
    wizardDismissed.value = false;
    await refreshInternal(true);
    toast("info", `Папка flowseal-zapret: ${resolved}`);
    return true;
  } catch (error) {
    toast("error", String(error));
    return false;
  }
}

export async function useDefaultRuntimeFolder(): Promise<boolean> {
  try {
    const resolved = await invoke<string>("set_runtime_path", { path: "" });
    wizardDismissed.value = false;
    await refreshInternal(true);
    toast("info", `Папка flowseal-zapret: ${resolved}`);
    return true;
  } catch (error) {
    toast("error", String(error));
    return false;
  }
}

export async function toggleAutostart(enabled: boolean) {
  try {
    await invoke("set_autostart", { enabled });
    await refreshInternal(true);
  } catch (error) {
    toast("error", String(error));
  }
}

export async function toggleCheckUpdates(enabled: boolean) {
  try {
    await invoke("set_check_updates", { enabled });
    await refreshInternal(true);
  } catch (error) {
    toast("error", String(error));
  }
}

export async function toggleFlashscore(enabled: boolean) {
  try {
    await invoke("set_flashscore_bypass", { enabled });
    await refreshInternal(true);
  } catch (error) {
    toast("error", String(error));
  }
}

export async function setAutoApply(enabled: boolean) {
  try {
    await invoke("set_bench_auto_apply", { enabled });
    await refreshInternal(true);
  } catch (error) {
    toast("error", String(error));
  }
}

export async function setGameFilter(mode: string) {
  try {
    await invoke("set_game_filter", { mode });
    await refreshInternal(true);
  } catch (error) {
    toast("error", String(error));
  }
}

export async function openFolder() {
  try {
    await invoke("open_runtime_folder");
  } catch (error) {
    toast("error", String(error));
  }
}

// ---------- Telegram proxy ----------

/** Loads everything the Telegram page renders: state, link and log tail. */
export async function refreshTgws() {
  try {
    const [nextStatus, logs] = await Promise.all([
      invoke<TgwsStatus>("get_tgws_status"),
      invoke<string[]>("get_tgws_logs"),
    ]);
    tgwsStatus.value = nextStatus;
    tgwsLogs.value = tgwsLogTail(logs);
    await refreshTgwsLink();
  } catch (error) {
    console.warn("Не удалось обновить состояние Telegram-прокси", error);
  }
}

async function refreshTgwsLink() {
  try {
    tgwsLink.value = await invoke<string>("get_tgws_link");
  } catch {
    tgwsLink.value = "";
  }
}

export async function checkTgwsUpdate(silent = true) {
  if (tgwsChecking.value) return;
  tgwsChecking.value = true;
  try {
    const info = await invoke<TgwsUpdateInfo>("check_tgws_update", { force: false });
    tgwsUpdate.value = info;
    if (!silent) {
      toast(
        info.has_update ? "info" : "info",
        info.has_update && info.latest
          ? `Доступно обновление прокси ${info.latest}.`
          : "Обновлений Telegram-прокси нет.",
      );
    }
  } catch (error) {
    if (!silent) toast("error", String(error));
  } finally {
    tgwsChecking.value = false;
  }
}

async function downloadTgwsBinary(): Promise<void> {
  tgwsProgress.value = "Проверка релиза…";
  const info = await invoke<TgwsUpdateInfo>("check_tgws_update", { force: true });
  tgwsUpdate.value = info;
  if (!info.asset_url || !info.latest) {
    throw new Error(
      "Релиз прокси ещё не опубликован. Бинарник публикуется тегом tgws-v* в этом репозитории.",
    );
  }
  if (!info.digest) {
    throw new Error("В релизе нет контрольной суммы SHA-256 — установка отменена.");
  }
  tgwsProgress.value = "Скачивание прокси…";
  await invoke("install_tgws_update", {
    url: info.asset_url,
    version: info.latest,
    digest: info.digest,
  });
  tgwsProgress.value = "Готово";
}

export async function toggleTgws() {
  if (tgwsBusy.value) return;
  const runningNow = tgwsStatus.value?.state === "running";
  tgwsBusy.value = true;
  try {
    if (runningNow) {
      await invoke("stop_tgws");
      toast("info", "Telegram-прокси остановлен.");
    } else {
      if (!tgwsStatus.value?.binary_present) {
        await downloadTgwsBinary();
      }
      await invoke("start_tgws");
      toast("success", "Telegram-прокси включён.");
    }
  } catch (error) {
    toast("error", String(error));
  } finally {
    await refreshTgws();
    tgwsBusy.value = false;
    tgwsProgress.value = null;
  }
}

export async function restartTgws() {
  if (tgwsBusy.value) return;
  tgwsBusy.value = true;
  try {
    await invoke("restart_tgws");
    toast("success", "Telegram-прокси перезапущен.");
  } catch (error) {
    toast("error", String(error));
  } finally {
    await refreshTgws();
    tgwsBusy.value = false;
  }
}

export async function applyTgwsUpdate() {
  if (tgwsBusy.value) return;
  tgwsBusy.value = true;
  try {
    await downloadTgwsBinary();
    await refreshTgws();
    await checkTgwsUpdate();
    toast("success", "Telegram-прокси обновлён.");
  } catch (error) {
    toast("error", String(error));
  } finally {
    tgwsBusy.value = false;
    tgwsProgress.value = null;
  }
}

export async function dismissTgwsUpdate() {
  const tag = tgwsUpdate.value?.latest;
  if (!tag) return;
  try {
    await invoke("dismiss_tgws_update", { tag });
    if (tgwsUpdate.value) tgwsUpdate.value = { ...tgwsUpdate.value, dismissed: true };
  } catch (error) {
    toast("error", String(error));
  }
}

export async function regenerateTgwsSecret() {
  if (tgwsBusy.value) return;
  tgwsBusy.value = true;
  try {
    await invoke<string>("regenerate_tgws_secret");
    await refreshTgws();
    toast("success", "Ключ и ссылка изменены.");
  } catch (error) {
    toast("error", String(error));
  } finally {
    tgwsBusy.value = false;
  }
}

export async function setTgwsHost(host: string) {
  try {
    const saved = await invoke<string>("set_tgws_host", { host });
    await refreshInternal(true);
    await refreshTgws();
    return saved;
  } catch (error) {
    toast("error", String(error));
    throw error;
  }
}

export async function setTgwsPort(port: number) {
  try {
    const saved = await invoke<number>("set_tgws_port", { port });
    await refreshInternal(true);
    await refreshTgws();
    return saved;
  } catch (error) {
    toast("error", String(error));
    throw error;
  }
}

export async function setTgwsSecret(secret: string) {
  try {
    const saved = await invoke<string>("set_tgws_secret", { secret });
    await refreshInternal(true);
    await refreshTgws();
    return saved;
  } catch (error) {
    toast("error", String(error));
    throw error;
  }
}

export async function toggleTgwsAutostart(enabled: boolean) {
  try {
    await invoke("set_tgws_autostart", { enabled });
    await refreshInternal(true);
  } catch (error) {
    toast("error", String(error));
  }
}

export async function copyTgwsLink() {
  if (!tgwsLink.value) return;
  try {
    await invoke("copy_to_clipboard", { text: tgwsLink.value });
    toast("success", "Ссылка скопирована.");
  } catch (error) {
    toast("error", String(error));
  }
}

export async function openTgwsInTelegram() {
  if (!tgwsLink.value) return;
  try {
    await invoke("open_telegram_link", { url: tgwsLink.value });
  } catch (error) {
    toast("error", String(error));
  }
}

export async function openTgwsLog() {
  try {
    await invoke("open_tgws_log");
  } catch (error) {
    toast("error", String(error));
  }
}
