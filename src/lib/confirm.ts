import { reactive } from "vue";

export interface ConfirmOptions {
  title: string;
  text?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  checkboxLabel?: string;
  checkboxValue?: boolean;
}

export interface ConfirmResult {
  ok: boolean;
  checked: boolean;
}

interface ConfirmState extends ConfirmOptions {
  open: boolean;
  checked: boolean;
  resolve: ((result: ConfirmResult) => void) | null;
}

export const confirmState = reactive<ConfirmState>({
  open: false,
  title: "",
  text: "",
  confirmLabel: "Подтвердить",
  cancelLabel: "Отмена",
  danger: false,
  checkboxLabel: undefined,
  checkboxValue: false,
  checked: false,
  resolve: null,
});

export function confirmAction(options: ConfirmOptions): Promise<ConfirmResult> {
  // A second confirmation must not leave the first caller's promise pending.
  if (confirmState.resolve) {
    const previous = confirmState.resolve;
    confirmState.resolve = null;
    confirmState.open = false;
    previous({ ok: false, checked: confirmState.checked });
  }
  return new Promise((resolve) => {
    Object.assign(confirmState, {
      ...options,
      open: true,
      checked: options.checkboxValue ?? false,
      resolve,
    });
  });
}

export function resolveConfirm(ok: boolean) {
  const resolve = confirmState.resolve;
  const checked = confirmState.checked;
  confirmState.open = false;
  confirmState.resolve = null;
  resolve?.({ ok, checked });
}
