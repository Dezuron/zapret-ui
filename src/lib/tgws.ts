import type { TgwsState } from "../types";

/** Human label for the supervisor state shown on the Telegram page. */
export function tgwsStateLabel(state: TgwsState | undefined): string {
  switch (state) {
    case "running":
      return "Прокси включён";
    case "starting":
      return "Запуск…";
    case "stopping":
      return "Остановка…";
    case "error":
      return "Ошибка";
    case "not_installed":
      return "Не установлен";
    default:
      return "Прокси выключен";
  }
}

export type StatusTone = "ok" | "idle" | "warn" | "danger";

export function tgwsTone(state: TgwsState | undefined): StatusTone {
  switch (state) {
    case "running":
      return "ok";
    case "starting":
    case "stopping":
      return "warn";
    case "error":
      return "danger";
    case "not_installed":
      return "warn";
    default:
      return "idle";
  }
}

/**
 * The proxy binds `0.0.0.0` to listen on every interface, but a Telegram link
 * must point at an address the client can dial; loopback is the sane default.
 */
export function displayHost(host: string | undefined): string {
  const value = (host ?? "").trim();
  if (!value || value === "0.0.0.0" || value === "::" || value === "[::]") return "127.0.0.1";
  return value;
}

export function tgwsEndpoint(host: string | undefined, port: number | undefined): string {
  const value = displayHost(host);
  const rendered = value.includes(":") && !value.startsWith("[") ? `[${value}]` : value;
  return `${rendered}:${port ?? 1443}`;
}

/** Version shown for the proxy binary: tag, "вручную" or "не установлен". */
export function tgwsVersionLabel(
  installedVersion: string | undefined | null,
  binaryPresent: boolean | undefined,
): string {
  const version = (installedVersion ?? "").trim();
  if (version) return version;
  return binaryPresent ? "вручную" : "не установлен";
}

/** Keeps the in-memory log buffer bounded, mirroring the backend's ring buffer. */
export function tgwsLogTail(lines: string[], limit = 300): string[] {
  if (lines.length <= limit) return [...lines];
  return lines.slice(lines.length - limit);
}

/** Whether changing `host` exposes the proxy beyond this machine. */
export function isPublicBinding(host: string | undefined): boolean {
  const value = displayHost(host);
  return value !== "127.0.0.1" && value !== "::1" && value !== "localhost";
}
