export interface StrategyInfo {
  name: string;
  file_name: string;
  active: boolean;
  args_preview: string;
}

export interface GeneratedInfo {
  name: string;
  parent: string;
  score: number | null;
  active: boolean;
  args_preview: string;
}

export interface AppStatus {
  app_version: string;
  strategies: StrategyInfo[];
  generated: GeneratedInfo[];
  current_strategy: string;
  running: boolean;
  installed_version: string | null;
  runtime_valid: boolean;
}

export interface ConfigView {
  autostart: boolean;
  check_updates: boolean;
  check_app_updates: boolean;
  flashscore_bypass: boolean;
  game_filter: string;
  runtime_version: string;
  runtime_path: string;
  theme: string;
  bench_budget: number;
  bench_auto_apply: boolean;
  tgws: TgwsConfigView;
}

export interface TgwsConfigView {
  autostart: boolean;
  host: string;
  port: number;
  secret: string;
  installed_version: string;
}

export type TgwsState = "stopped" | "starting" | "running" | "stopping" | "error" | "not_installed";

export interface TgwsStatus {
  state: TgwsState;
  pid: number | null;
  last_error: string | null;
  installed_version: string;
  binary_present: boolean;
  port_conflict: boolean;
}

export interface TgwsNotice {
  ok: boolean;
  message: string;
}

export interface TgwsUpdateInfo {
  installed: string;
  latest: string | null;
  has_update: boolean;
  asset_name: string | null;
  asset_url: string | null;
  asset_size: number;
  digest: string | null;
  release_url: string | null;
  published_at: string | null;
  dismissed: boolean;
}

export interface AppUpdateInfo {
  current: string;
  latest: string;
  available: boolean;
  url: string;
  notes: string;
  size: number;
  can_auto_update: boolean;
}

export interface UpdateInfo {
  installed: string | null;
  latest: string;
  update_available: boolean;
  changelog: string;
  download_size: number;
}

export interface WhatsNew {
  version: string;
  notes: string;
  url: string;
}

export type HostStatus = "ok" | "ssl" | "reset" | "timeout" | "dns" | "unsupported" | "error";

export type HostGroup = "discord" | "youtube" | "control";

export type Verdict = "works" | "partial" | "broken" | "disqualified";

export type BaselineGate = "notblocked" | "blocked" | "unmeasurable";

export interface HostProbe {
  name: string;
  url: string;
  group: HostGroup;
  status: HostStatus;
  ms: number;
  detail: string;
}

export interface CandidateResult {
  name: string;
  source: "official" | "generated";
  parent: string | null;
  score: number;
  verdict: Verdict;
  discord_ok: number;
  discord_total: number;
  youtube_ok: number;
  youtube_total: number;
  control_ok: number;
  control_total: number;
  avg_ms: number;
  error: string | null;
  hosts: HostProbe[];
}

export interface BenchSummary {
  results: CandidateResult[];
  baseline: HostProbe[];
  gate: BaselineGate;
  cancelled: boolean;
  best: string | null;
  applied: string | null;
  message: string | null;
  ran_at: number;
}

export interface BenchProgress {
  phase: string;
  index: number;
  total: number;
  name: string;
  percent: number;
  message: string;
}

export interface QuickCheck {
  running: boolean;
  score: number;
  verdict: Verdict;
  gate: BaselineGate;
  hosts: HostProbe[];
}
