import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppStatus, ConfigView } from "../types";
import { benchRunning, busy, status, whatsNew, wizardDismissed } from "./state";

const { invokeMock, listenMock, openMock, confirmMock, toastMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  openMock: vi.fn(),
  confirmMock: vi.fn(),
  toastMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));
vi.mock("../lib/confirm", () => ({ confirmAction: confirmMock }));
vi.mock("../lib/toast", () => ({ toast: toastMock }));

import { checkUpdate, chooseRuntimeFolder, dismissWhatsNew } from "./actions";

const statusFixture: AppStatus = {
  app_version: "1.2.0",
  strategies: [],
  generated: [],
  current_strategy: "auto-87",
  running: true,
  installed_version: "1.10.4",
  runtime_valid: true,
};

const configFixture: ConfigView = {
  autostart: true,
  check_updates: true,
  check_app_updates: true,
  flashscore_bypass: true,
  game_filter: "disabled",
  runtime_version: "1.10.4",
  runtime_path: "C:\\old\\path",
  theme: "dark",
  bench_budget: 100,
  bench_auto_apply: true,
  tgws: {
    autostart: true,
    host: "127.0.0.1",
    port: 1443,
    secret: "00112233445566778899aabbccddeeff",
    installed_version: "",
  },
};

describe("folder selection", () => {
  beforeEach(() => {
    status.value = statusFixture;
    busy.value = false;
    benchRunning.value = false;
    wizardDismissed.value = true;
    whatsNew.value = null;
    invokeMock.mockReset();
    openMock.mockReset();
    confirmMock.mockReset();
    toastMock.mockReset();

    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_state") return { ...statusFixture, running: false };
      if (command === "get_config") return configFixture;
      return null;
    });
  });

  it("stops a running bypass before changing the folder", async () => {
    confirmMock.mockResolvedValue({ ok: true });
    openMock.mockResolvedValue("C:\\new\\path");
    invokeMock.mockImplementation(async (command: string, args?: { path?: string }) => {
      if (command === "get_state") return { ...statusFixture, running: false };
      if (command === "get_config") return configFixture;
      if (command === "set_runtime_path") return args?.path;
      return null;
    });

    await expect(chooseRuntimeFolder()).resolves.toBe(true);

    expect(invokeMock).toHaveBeenCalledWith("stop");
    expect(invokeMock).toHaveBeenCalledWith("set_runtime_path", { path: "C:\\new\\path" });
    expect(toastMock).toHaveBeenCalledWith("info", "Папка flowseal-zapret: C:\\new\\path");
  });

  it("does not stop anything when the user cancels", async () => {
    confirmMock.mockResolvedValue({ ok: false });

    await expect(chooseRuntimeFolder()).resolves.toBe(false);
    expect(invokeMock).not.toHaveBeenCalledWith("stop");
  });

  it("acknowledges What's new and clears it", async () => {
    whatsNew.value = { version: "1.3.0", notes: "- test", url: "https://example" };

    await dismissWhatsNew();

    expect(invokeMock).toHaveBeenCalledWith("dismiss_whats_new");
    expect(whatsNew.value).toBeNull();
  });

  it("reports no flowseal-zapret update only for a manual check", async () => {
    invokeMock.mockResolvedValue({
      installed: "1.10.4",
      latest: "1.10.4",
      update_available: false,
      changelog: "",
      download_size: 0,
    });

    await checkUpdate(false);
    expect(toastMock).toHaveBeenCalledWith("info", "Обновлений flowseal-zapret нет.");

    toastMock.mockReset();
    await checkUpdate(true);
    expect(toastMock).not.toHaveBeenCalled();
  });
});
