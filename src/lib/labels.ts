import type { HostStatus, Verdict } from "../types";

export const verdictLabel: Record<Verdict, string> = {
  works: "работает",
  partial: "частично",
  broken: "не работает",
  disqualified: "ломает интернет",
};

export const hostStatusLabel: Record<HostStatus, string> = {
  ok: "доступен",
  ssl: "подмена сертификата",
  reset: "сброс соединения",
  timeout: "таймаут",
  dns: "ошибка DNS",
  unsupported: "не поддерживается",
  error: "ошибка",
};

export function formatBytes(bytes: number): string {
  if (!bytes) return "";
  return bytes > 1024 * 1024
    ? (bytes / 1024 / 1024).toFixed(1) + " МБ"
    : (bytes / 1024).toFixed(0) + " КБ";
}

export function formatTimestamp(seconds: number): string {
  if (!seconds) return "";
  return new Intl.DateTimeFormat("ru-RU", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(seconds * 1000));
}
