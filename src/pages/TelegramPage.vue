<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import {
  AlertTriangle,
  Copy,
  Download,
  ExternalLink,
  RefreshCw,
  RotateCw,
  ScrollText,
  Send,
} from "lucide-vue-next";
import Card from "../components/ui/card/Card.vue";
import CardContent from "../components/ui/card/CardContent.vue";
import CardDescription from "../components/ui/card/CardDescription.vue";
import CardHeader from "../components/ui/card/CardHeader.vue";
import CardTitle from "../components/ui/card/CardTitle.vue";
import Button from "../components/ui/button/Button.vue";
import Switch from "../components/ui/switch/Switch.vue";
import Toggle from "../components/Toggle.vue";
import { formatBytes } from "../lib/labels";
import {
  displayHost,
  isPublicBinding,
  tgwsEndpoint,
  tgwsTone,
  tgwsVersionLabel,
} from "../lib/tgws";
import {
  applyTgwsUpdate,
  checkTgwsUpdate,
  config,
  copyTgwsLink,
  dismissTgwsUpdate,
  openTgwsInTelegram,
  openTgwsLog,
  refreshTgws,
  regenerateTgwsSecret,
  restartTgws,
  setTgwsHost,
  setTgwsPort,
  setTgwsSecret,
  switchTgwsPort,
  tgwsBusy,
  tgwsChecking,
  tgwsHasUpdate,
  tgwsLink,
  tgwsLogs,
  tgwsProgress,
  tgwsRunning,
  tgwsStateText,
  tgwsStatus,
  tgwsUpdate,
  toggleTgws,
  toggleTgwsAutostart,
} from "../store";

const hostDraft = ref("");
const portDraft = ref(1555);
const secretDraft = ref("");
const logsOpen = ref(false);
const logBox = ref<HTMLElement | null>(null);

const tone = computed(() => tgwsTone(tgwsStatus.value?.state));
const endpointLabel = computed(() =>
  tgwsEndpoint(config.value?.tgws.host, config.value?.tgws.port),
);
const versionLabel = computed(() =>
  tgwsVersionLabel(
    config.value?.tgws.installed_version || tgwsStatus.value?.installed_version,
    tgwsStatus.value?.binary_present,
  ),
);
const transitioning = computed(
  () => tgwsStatus.value?.state === "starting" || tgwsStatus.value?.state === "stopping",
);
const publicBinding = computed(() => isPublicBinding(config.value?.tgws.host));
const logText = computed(() => tgwsLogs.value.join("\n"));

watch(
  () => config.value?.tgws,
  (tgws) => {
    if (!tgws) return;
    hostDraft.value = tgws.host;
    portDraft.value = tgws.port;
    secretDraft.value = tgws.secret;
  },
  { immediate: true, deep: true },
);

watch(logText, async () => {
  if (!logsOpen.value) return;
  await nextTick();
  logBox.value?.scrollTo({ top: logBox.value.scrollHeight });
});

onMounted(async () => {
  await refreshTgws();
  await checkTgwsUpdate(true);
});

async function commitHost() {
  const value = hostDraft.value.trim();
  if (!value || value === config.value?.tgws.host) return;
  try {
    hostDraft.value = await setTgwsHost(value);
  } catch {
    hostDraft.value = config.value?.tgws.host ?? "127.0.0.1";
  }
}

async function commitPort() {
  const value = Number(portDraft.value);
  if (!Number.isFinite(value) || value === config.value?.tgws.port) return;
  try {
    portDraft.value = await setTgwsPort(value);
  } catch {
    portDraft.value = config.value?.tgws.port ?? 1555;
  }
}

async function commitSecret() {
  const value = secretDraft.value.trim();
  if (!value || value === config.value?.tgws.secret) return;
  try {
    secretDraft.value = await setTgwsSecret(value);
  } catch {
    secretDraft.value = config.value?.tgws.secret ?? "";
  }
}
</script>

<template>
  <div class="page-stack tgws-page">
    <Card class="tgws-card tgws-hero">
      <CardHeader class="tgws-hero-head">
        <div class="tgws-hero-icon"><Send :size="21" /></div>
        <div class="tgws-hero-text">
          <span class="eyebrow">Telegram</span>
          <CardTitle>WS-прокси</CardTitle>
          <CardDescription>{{ tgwsStateText }}</CardDescription>
        </div>
        <div class="tgws-hero-controls">
          <span class="tgws-dot" :class="`is-${tone}`" aria-hidden="true"></span>
          <Switch
            :model-value="tgwsRunning"
            :disabled="tgwsBusy || transitioning"
            aria-label="Включить или выключить Telegram-прокси"
            @update:model-value="toggleTgws"
          />
        </div>
      </CardHeader>
      <CardContent class="tgws-hero-content">
        <div class="tgws-meta">
          <div>
            <span>Адрес</span>
            <strong>{{ endpointLabel }}</strong>
          </div>
          <div>
            <span>Версия</span>
            <strong>{{ versionLabel }}</strong>
          </div>
          <div>
            <span>PID</span>
            <strong>{{ tgwsStatus?.pid ?? "—" }}</strong>
          </div>
        </div>

        <div class="tgws-actions">
          <Button
            variant="outline"
            size="sm"
            :disabled="tgwsBusy || transitioning || !tgwsRunning"
            @click="restartTgws"
          >
            <RotateCw :size="14" />
            Перезапустить
          </Button>
          <Button variant="ghost" size="sm" @click="openTgwsLog">
            <ScrollText :size="14" />
            Открыть лог
          </Button>
          <Button
            variant="ghost"
            size="sm"
            :disabled="tgwsChecking"
            @click="checkTgwsUpdate(false, true)"
          >
            <RefreshCw :size="14" :class="{ spinning: tgwsChecking }" />
            {{ tgwsChecking ? "Проверка…" : "Проверить обновление" }}
          </Button>
        </div>

        <p v-if="tgwsProgress" class="tgws-progress">{{ tgwsProgress }}</p>
        <div v-if="tgwsStatus?.last_error" class="tgws-error">
          <AlertTriangle :size="14" />
          <div class="tgws-error-body">
            <span>{{ tgwsStatus.last_error }}</span>
            <Button
              v-if="tgwsStatus?.port_conflict"
              variant="outline"
              size="sm"
              :disabled="tgwsBusy"
              @click="switchTgwsPort"
            >
              Занять свободный порт
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>

    <Card
      v-if="tgwsRunning && (tgwsStatus?.client_connections ?? 0) === 0"
      class="tgws-card tgws-connect-hint"
    >
      <AlertTriangle :size="18" />
      <div class="tgws-connect-hint-text">
        <strong>Telegram ещё не использует прокси</strong>
        <span>
          В Telegram выберите «Использовать собственный прокси» и добавьте ссылку ниже — проще всего
          кнопкой «Подключить Telegram»: ключ и порт уже верные, менять ничего не нужно.
        </span>
      </div>
      <Button size="sm" :disabled="!tgwsLink" @click="openTgwsInTelegram">
        Подключить Telegram
      </Button>
    </Card>

    <Card v-if="tgwsHasUpdate && tgwsUpdate?.latest" class="tgws-card tgws-update">
      <CardContent class="tgws-update-content">
        <div class="tgws-update-icon"><Download :size="18" /></div>
        <div class="tgws-update-text">
          <strong>Доступно обновление прокси {{ tgwsUpdate.latest }}</strong>
          <span>
            Установлено: {{ tgwsUpdate.installed || "неизвестно" }}
            <template v-if="tgwsUpdate.asset_size">
              · {{ formatBytes(tgwsUpdate.asset_size) }}
            </template>
          </span>
        </div>
        <div class="tgws-update-actions">
          <Button variant="ghost" size="sm" :disabled="tgwsBusy" @click="dismissTgwsUpdate">
            Позже
          </Button>
          <Button size="sm" :disabled="tgwsBusy" @click="applyTgwsUpdate">
            <Download :size="14" />
            Обновить
          </Button>
        </div>
      </CardContent>
    </Card>

    <Card class="tgws-card">
      <CardHeader>
        <CardTitle>Подключение</CardTitle>
        <CardDescription>
          Вставьте ссылку в Telegram → Настройки → Продвинутые настройки → Тип соединения, либо
          откройте её кнопкой справа.
        </CardDescription>
      </CardHeader>
      <CardContent class="tgws-connect">
        <div class="tgws-link-row">
          <code class="tgws-link" :title="tgwsLink">{{
            tgwsLink || "Ключ ещё не сгенерирован"
          }}</code>
          <Button
            variant="outline"
            size="icon"
            title="Копировать ссылку"
            aria-label="Копировать ссылку"
            :disabled="!tgwsLink"
            @click="copyTgwsLink"
          >
            <Copy :size="15" />
          </Button>
          <Button
            variant="outline"
            size="icon"
            title="Открыть в Telegram"
            aria-label="Открыть ссылку в Telegram"
            :disabled="!tgwsLink"
            @click="openTgwsInTelegram"
          >
            <ExternalLink :size="15" />
          </Button>
          <Button
            variant="outline"
            size="icon"
            title="Сменить ключ и ссылку"
            aria-label="Сменить ключ и ссылку"
            :disabled="tgwsBusy"
            @click="regenerateTgwsSecret"
          >
            <RefreshCw :size="15" :class="{ spinning: tgwsBusy }" />
          </Button>
        </div>
        <p class="tgws-hint">
          Ключ живёт в <code>config.json</code> и не меняется между запусками. Смена ключа сразу
          перезапускает прокси, старые ссылки перестают работать.
        </p>
      </CardContent>
    </Card>

    <Card class="tgws-card">
      <CardHeader>
        <CardTitle>Параметры</CardTitle>
        <CardDescription>
          Остальные флаги прокси (DC-адреса, буфер, пул, Cloudflare) настраиваются в секции
          <code>tgws</code> файла <code>config.json</code>.
        </CardDescription>
      </CardHeader>
      <CardContent class="tgws-fields">
        <label class="field">
          <span class="field-label">Хост</span>
          <input
            v-model="hostDraft"
            class="tgws-input"
            spellcheck="false"
            placeholder="127.0.0.1"
            @change="commitHost"
            @keydown.enter="commitHost"
          />
        </label>
        <label class="field">
          <span class="field-label">Порт</span>
          <input
            v-model.number="portDraft"
            class="tgws-input"
            type="number"
            min="1024"
            max="65535"
            @change="commitPort"
            @keydown.enter="commitPort"
          />
        </label>
        <label class="field field-wide">
          <span class="field-label">Секретный ключ (32 hex)</span>
          <input
            v-model="secretDraft"
            class="tgws-input tgws-mono"
            spellcheck="false"
            maxlength="32"
            @change="commitSecret"
            @keydown.enter="commitSecret"
          />
        </label>

        <Toggle
          class="field-wide"
          label="Запускать прокси вместе с приложением"
          hint="Порядок адресов Telegram: сначала прокси, потом клиент"
          :checked="config?.tgws.autostart"
          :disabled="tgwsBusy"
          @change="toggleTgwsAutostart"
        />

        <p v-if="publicBinding" class="tgws-warning field-wide">
          <AlertTriangle :size="14" />
          <span>
            Хост <strong>{{ displayHost(config?.tgws.host) }}</strong> открывает прокси другим
            устройствам в сети. Оставьте 127.0.0.1, если доступ извне не нужен.
          </span>
        </p>
      </CardContent>
    </Card>

    <Card class="tgws-card">
      <CardHeader class="tgws-log-head">
        <div>
          <CardTitle>Логи</CardTitle>
          <CardDescription
            >Последние {{ tgwsLogs.length }} строк от процесса прокси</CardDescription
          >
        </div>
        <Button variant="ghost" size="sm" @click="logsOpen = !logsOpen">
          {{ logsOpen ? "Свернуть" : "Показать" }}
        </Button>
      </CardHeader>
      <CardContent v-if="logsOpen">
        <pre ref="logBox" class="tgws-log">{{ logText || "Пока пусто — включите прокси." }}</pre>
      </CardContent>
    </Card>
  </div>
</template>

<style scoped>
.tgws-page {
  padding-bottom: 8px;
}

/* Same glass treatment as the panels on the overview and settings pages. */
.tgws-card {
  border-color: var(--glass-border);
  background: linear-gradient(160deg, var(--glass-bg), var(--glass-bg-2));
  box-shadow:
    0 18px 50px var(--glass-shadow),
    inset 0 1px 0 var(--glass-highlight);
  backdrop-filter: blur(18px) saturate(135%);
}

/* The proxy is running, but no Telegram client has connected yet. */
.tgws-connect-hint {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 16px;
  border-color: color-mix(in oklab, var(--warn) 45%, transparent);
  background: var(--warn-soft);
  color: var(--warn);
}

.tgws-connect-hint-text {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.tgws-connect-hint-text strong {
  font-size: 12.5px;
}

.tgws-connect-hint-text span {
  color: var(--text-2);
  font-size: 11.5px;
  line-height: 1.45;
}

.tgws-hero-head {
  display: flex;
  align-items: center;
  gap: 12px;
}

.tgws-hero-icon {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  flex: 0 0 auto;
  border-radius: 12px;
  border: 1px solid var(--glass-border);
  background: var(--accent-soft);
  color: var(--accent-text);
}

.tgws-hero-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.tgws-hero-controls {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-left: auto;
}

.tgws-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-3);
}

.tgws-dot.is-ok {
  background: var(--ok);
  box-shadow: 0 0 0 3px var(--ok-soft);
}

.tgws-dot.is-warn {
  background: var(--warn);
  box-shadow: 0 0 0 3px var(--warn-soft);
}

.tgws-dot.is-danger {
  background: var(--danger);
  box-shadow: 0 0 0 3px var(--danger-soft);
}

.tgws-hero-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.tgws-meta {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
}

.tgws-meta div {
  min-width: 0;
  padding: 9px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--muted);
}

.tgws-meta span {
  display: block;
  color: var(--text-3);
  font-size: 9px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.tgws-meta strong {
  display: block;
  overflow: hidden;
  color: var(--text);
  font-family: var(--mono);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tgws-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.tgws-progress {
  color: var(--text-2);
  font-size: 12px;
}

.tgws-error,
.tgws-warning {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px 12px;
  border-radius: 9px;
  font-size: 12px;
  line-height: 1.45;
}

.tgws-error {
  border: 1px solid color-mix(in oklab, var(--danger) 40%, transparent);
  background: var(--danger-soft);
  color: var(--danger);
}

.tgws-error-body {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
  min-width: 0;
}

.tgws-warning {
  border: 1px solid color-mix(in oklab, var(--warn) 40%, transparent);
  background: var(--warn-soft);
  color: var(--warn);
}

.tgws-update-content {
  display: flex;
  align-items: center;
  gap: 12px;
  /* CardContent defaults to pt-0 (it normally follows a header); this banner
     has no header, so it needs its own vertical rhythm. */
  padding: 14px 16px;
}

.tgws-update-icon {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  flex: 0 0 auto;
  border-radius: 10px;
  background: var(--accent-soft);
  color: var(--accent-text);
}

.tgws-update-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.tgws-update-text strong {
  font-size: 12.5px;
}

.tgws-update-text span {
  color: var(--text-3);
  font-size: 11px;
}

.tgws-update-actions {
  display: flex;
  gap: 8px;
  margin-left: auto;
}

.tgws-connect {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.tgws-link-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tgws-link {
  flex: 1 1 auto;
  min-width: 0;
  overflow-x: auto;
  padding: 9px 11px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--muted);
  color: var(--text-2);
  font-family: var(--mono);
  font-size: 11px;
  white-space: nowrap;
}

.tgws-hint {
  color: var(--text-3);
  font-size: 11px;
  line-height: 1.5;
}

.tgws-hint code {
  color: var(--text-2);
  font-family: var(--mono);
  font-size: 10.5px;
}

.tgws-fields {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 140px);
  gap: 12px;
}

.tgws-fields .field-wide {
  grid-column: 1 / -1;
}

.tgws-input {
  width: 100%;
  height: 34px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--secondary);
  color: var(--text);
  font: inherit;
  font-size: 12px;
}

.tgws-input::placeholder {
  color: var(--text-3);
}

.tgws-input:focus-visible {
  outline: none;
  border-color: var(--primary);
  box-shadow: var(--ring);
}

.tgws-mono {
  font-family: var(--mono);
  font-size: 11px;
}

.tgws-log-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.tgws-log {
  max-height: 240px;
  margin: 0;
  overflow: auto;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--secondary);
  color: var(--text-2);
  font-family: var(--mono);
  font-size: 11px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}

.spinning {
  animation: tgws-spin 0.9s linear infinite;
}

@keyframes tgws-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 860px) {
  .tgws-meta {
    grid-template-columns: minmax(0, 1fr);
  }

  .tgws-fields {
    grid-template-columns: minmax(0, 1fr);
  }

  .tgws-link-row {
    flex-wrap: wrap;
  }
}
</style>
