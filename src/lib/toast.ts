import { ref } from "vue";

export type ToastKind = "info" | "success" | "warning" | "error";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  text: string;
}

export const toasts = ref<ToastItem[]>([]);

let sequence = 0;

export function toast(kind: ToastKind, text: string, ttl = 7000) {
  const id = ++sequence;
  toasts.value = [...toasts.value.slice(-3), { id, kind, text }];
  window.setTimeout(() => dismissToast(id), ttl);
}

export function dismissToast(id: number) {
  toasts.value = toasts.value.filter((item) => item.id !== id);
}
