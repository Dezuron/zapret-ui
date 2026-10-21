import { computed, ref } from "vue";
import type {
  AppStatus,
  AppUpdateInfo,
  BenchProgress,
  BenchSummary,
  CandidateResult,
  ConfigView,
  QuickCheck,
  UpdateInfo,
  WhatsNew,
} from "../types";
import { formatTimestamp } from "../lib/labels";

export type SortMode = "name" | "score" | "latency";

export const status = ref<AppStatus | null>(null);
export const config = ref<ConfigView | null>(null);
export const updateInfo = ref<UpdateInfo | null>(null);
export const checkingUpdate = ref(false);
export const updateProgress = ref<string | null>(null);
export const appUpdate = ref<AppUpdateInfo | null>(null);
export const checkingAppUpdate = ref(false);
export const appUpdateProgress = ref<string | null>(null);
export const wizardDismissed = ref(false);
export const whatsNew = ref<WhatsNew | null>(null);
export const busy = ref(false);
export const selected = ref("");
export const starting = ref(false);

export const benchSummary = ref<BenchSummary | null>(null);
export const benchProgress = ref<BenchProgress | null>(null);
export const benchRunning = ref(false);
export const benchBusy = ref(false);
export const quick = ref<QuickCheck | null>(null);
export const includeGenerated = ref(false);
export const budget = ref(40);
export const sortMode = ref<SortMode>("score");

export const running = computed(() => status.value?.running ?? false);
export const runtimeReady = computed(() => status.value?.runtime_valid ?? false);
export const runtimePath = computed(() => config.value?.runtime_path ?? "");
export const showSetupWizard = computed(
  () => !!status.value && !runtimeReady.value && !wizardDismissed.value,
);
export const showWhatsNew = computed(() => !!whatsNew.value && !showSetupWizard.value);
export const currentName = computed(() => status.value?.current_strategy ?? "");
export const appVersion = computed(() => status.value?.app_version ?? "");

export const statusText = computed(() => {
  if (!status.value) return "Загрузка…";
  if (!status.value.runtime_valid) return "flowseal-zapret не установлен";
  if (benchRunning.value) return "Идёт проверка стратегий";
  return running.value ? "Обход работает" : "Обход остановлен";
});

export const statusTone = computed<"ok" | "idle" | "danger">(() => {
  if (!status.value) return "idle";
  if (!status.value.runtime_valid) return "danger";
  if (benchRunning.value) return "idle";
  return running.value ? "ok" : "idle";
});

const benchByName = computed(() => {
  const map = new Map<string, CandidateResult>();
  for (const result of benchSummary.value?.results ?? []) {
    map.set(result.name, result);
  }
  return map;
});

function sortRows<T extends { name: string }>(rows: T[]): T[] {
  if (sortMode.value === "name") return rows;
  const collator = new Intl.Collator("ru");
  return [...rows].sort((a, b) => {
    const left = benchByName.value.get(a.name);
    const right = benchByName.value.get(b.name);
    if (sortMode.value === "score") {
      const leftScore = left?.score ?? -1;
      const rightScore = right?.score ?? -1;
      if (leftScore !== rightScore) return rightScore - leftScore;
    } else {
      const leftMs = left && left.avg_ms > 0 ? left.avg_ms : Number.MAX_SAFE_INTEGER;
      const rightMs = right && right.avg_ms > 0 ? right.avg_ms : Number.MAX_SAFE_INTEGER;
      if (leftMs !== rightMs) return leftMs - rightMs;
    }
    return collator.compare(a.name, b.name);
  });
}

export interface StrategyRowData {
  name: string;
  active: boolean;
  args_preview: string;
  generated: boolean;
  parent?: string;
  bench?: CandidateResult;
}

export const officialRows = computed<StrategyRowData[]>(() =>
  sortRows(status.value?.strategies ?? []).map((strategy) => ({
    ...strategy,
    generated: false,
    bench: benchByName.value.get(strategy.name),
  })),
);

export const generatedRows = computed<StrategyRowData[]>(() =>
  sortRows(status.value?.generated ?? []).map((strategy) => ({
    ...strategy,
    generated: true,
    bench: benchByName.value.get(strategy.name),
  })),
);

export const benchDate = computed(() =>
  benchSummary.value ? formatTimestamp(benchSummary.value.ran_at) : "",
);

export const benchHostCount = computed(() => benchSummary.value?.baseline.length ?? 0);

export const gateMessage = computed(() => {
  const gate = benchSummary.value?.gate;
  if (!gate) return "";
  if (gate === "notblocked")
    return "Без обхода цели доступны — DPI не блокирует Discord/YouTube, перебор не нужен.";
  if (gate === "unmeasurable") return "Не удалось замерить цели без обхода: проверьте DNS и сеть.";
  return "";
});
