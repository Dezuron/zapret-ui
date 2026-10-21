import { beforeEach, describe, expect, it } from "vitest";
import type { AppStatus } from "../types";
import {
  officialRows,
  runtimeReady,
  showSetupWizard,
  showWhatsNew,
  status,
  statusText,
  whatsNew,
  wizardDismissed,
} from "./state";

function makeStatus(overrides: Partial<AppStatus> = {}): AppStatus {
  return {
    app_version: "1.2.0",
    strategies: [],
    generated: [],
    current_strategy: "",
    running: false,
    installed_version: null,
    runtime_valid: false,
    ...overrides,
  };
}

describe("store state", () => {
  beforeEach(() => {
    status.value = null;
    wizardDismissed.value = false;
    whatsNew.value = null;
  });

  it("shows the setup wizard until zapret is installed or dismissed", () => {
    status.value = makeStatus();
    expect(runtimeReady.value).toBe(false);
    expect(showSetupWizard.value).toBe(true);
    expect(statusText.value).toBe("flowseal-zapret не установлен");

    wizardDismissed.value = true;
    expect(showSetupWizard.value).toBe(false);
  });

  it("reports a running bypass", () => {
    status.value = makeStatus({
      runtime_valid: true,
      running: true,
      installed_version: "1.10.4",
    });
    expect(runtimeReady.value).toBe(true);
    expect(showSetupWizard.value).toBe(false);
    expect(statusText.value).toBe("Обход работает");
  });

  it("shows What's new only after the setup wizard", () => {
    status.value = makeStatus();
    whatsNew.value = { version: "1.3.0", notes: "- test", url: "https://example" };
    expect(showWhatsNew.value).toBe(false);

    status.value = makeStatus({ runtime_valid: true, installed_version: "1.10.4" });
    expect(showWhatsNew.value).toBe(true);
  });

  it("sorts official strategies by score", () => {
    status.value = makeStatus({
      strategies: [
        { name: "alpha", file_name: "alpha.bat", active: false, args_preview: "a" },
        { name: "beta", file_name: "beta.bat", active: true, args_preview: "b" },
      ],
    });
    expect(officialRows.value.map((row) => row.name)).toEqual(["alpha", "beta"]);
  });
});
