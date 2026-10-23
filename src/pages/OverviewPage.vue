<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import { ArrowUpRight, RefreshCw, Send, ShieldCheck } from "lucide-vue-next";
import Button from "../components/ui/button/Button.vue";
import Card from "../components/ui/card/Card.vue";
import CardContent from "../components/ui/card/CardContent.vue";
import CardDescription from "../components/ui/card/CardDescription.vue";
import CardHeader from "../components/ui/card/CardHeader.vue";
import CardTitle from "../components/ui/card/CardTitle.vue";
import Switch from "../components/ui/switch/Switch.vue";
import {
  appUpdate,
  appVersion,
  applyAppUpdate,
  applyTgwsUpdate,
  applyUpdate,
  benchDate,
  benchSummary,
  benchRunning,
  busy,
  checkAppUpdate,
  checkTgwsUpdate,
  checkUpdate,
  checkingAppUpdate,
  checkingUpdate,
  config,
  running,
  runtimeReady,
  status,
  tgwsBusy,
  tgwsChecking,
  tgwsRunning,
  tgwsStateText,
  tgwsStatus,
  tgwsUpdate,
  toggleStart,
  toggleTgws,
  updateInfo,
} from "../store";
import { tgwsEndpoint, tgwsVersionLabel } from "../lib/tgws";
import { toast } from "../lib/toast";

const router = useRouter();

const zapretState = computed(() => {
  if (!runtimeReady.value) return "Не установлен";
  return running.value ? "Включён" : "Выключен";
});

const bestScore = computed(() => {
  const best = benchSummary.value?.best;
  if (!best) return null;
  return benchSummary.value?.results.find((result) => result.name === best)?.score ?? null;
});

const appHasUpdate = computed(() => !!appUpdate.value?.available);
const zapretHasUpdate = computed(() => !!updateInfo.value?.update_available);
const tgwsHasUpdate = computed(() => !!tgwsUpdate.value?.has_update);
const checkingAllUpdates = computed(
  () => checkingAppUpdate.value || checkingUpdate.value || tgwsChecking.value,
);

const telegramEndpoint = computed(() =>
  tgwsEndpoint(config.value?.tgws.host, config.value?.tgws.port),
);
const telegramVersion = computed(() =>
  tgwsVersionLabel(
    config.value?.tgws.installed_version || tgwsStatus.value?.installed_version,
    tgwsStatus.value?.binary_present,
  ),
);
const telegramHint = computed(() => {
  if (!tgwsRunning.value) return tgwsStateText.value;
  return (tgwsStatus.value?.client_connections_total ?? 0) > 0
    ? "Прокси включён"
    : "Прокси включён · Telegram не подключён";
});

async function checkAllUpdates() {
  if (checkingAllUpdates.value || benchRunning.value) return;
  const results = await Promise.all([
    checkAppUpdate(true),
    checkUpdate(true),
    checkTgwsUpdate(true, true),
  ]);
  if (results.some((ok) => !ok)) {
    toast("warning", "Не удалось проверить все обновления — проверьте сеть.");
    return;
  }

  const updates: string[] = [];
  if (appUpdate.value?.available) updates.push(`приложение ${appUpdate.value.latest}`);
  if (updateInfo.value?.update_available) {
    updates.push(`flowseal-zapret ${updateInfo.value.latest}`);
  }
  if (tgwsUpdate.value?.has_update && tgwsUpdate.value.latest) {
    updates.push(`Telegram-прокси ${tgwsUpdate.value.latest}`);
  }
  toast(
    updates.length ? "success" : "info",
    updates.length ? `Доступны обновления: ${updates.join(", ")}.` : "Обновлений нет.",
  );
}
</script>

<template>
  <div class="main-page">
    <div class="power-grid">
      <Card class="power-card">
        <CardHeader class="power-head">
          <div class="power-icon zapret"><ShieldCheck :size="20" /></div>
          <div class="power-heading">
            <CardTitle>Zapret</CardTitle>
            <CardDescription>{{ zapretState }}</CardDescription>
          </div>
          <Switch
            :model-value="running"
            :disabled="busy || benchRunning || !runtimeReady"
            aria-label="Включить или выключить обход Zapret"
            @update:model-value="toggleStart"
          />
        </CardHeader>
        <CardContent class="power-content">
          <div class="power-meta">
            <span>Стратегия</span>
            <strong>{{ status?.current_strategy || "не выбрана" }}</strong>
          </div>
          <div class="power-meta">
            <span>Версия</span>
            <strong>{{ status?.installed_version || "не установлена" }}</strong>
          </div>
          <Button
            v-if="!runtimeReady"
            class="power-action"
            variant="secondary"
            size="sm"
            @click="router.push('/settings')"
          >
            Настроить Zapret
          </Button>
          <Button
            v-else
            class="power-action"
            variant="outline"
            size="sm"
            @click="router.push('/zapret')"
          >
            Стратегии и проверка
            <ArrowUpRight :size="13" />
          </Button>
        </CardContent>
      </Card>

      <Card class="power-card">
        <CardHeader class="power-head">
          <div class="power-icon telegram"><Send :size="19" /></div>
          <div class="power-heading">
            <CardTitle>Telegram</CardTitle>
            <CardDescription>{{ telegramHint }}</CardDescription>
          </div>
          <Switch
            :model-value="tgwsRunning"
            :disabled="tgwsBusy"
            aria-label="Включить или выключить Telegram WS-прокси"
            @update:model-value="toggleTgws"
          />
        </CardHeader>
        <CardContent class="power-content">
          <div class="power-meta">
            <span>Адрес</span>
            <strong>{{ telegramEndpoint }}</strong>
          </div>
          <div class="power-meta">
            <span>Версия</span>
            <strong>{{ telegramVersion }}</strong>
          </div>
          <Button
            class="power-action"
            variant="outline"
            size="sm"
            @click="router.push('/telegram')"
          >
            Подключение и ключ
            <ArrowUpRight :size="13" />
          </Button>
        </CardContent>
      </Card>
    </div>

    <Card class="updates-card">
      <CardHeader class="updates-head">
        <div>
          <span class="eyebrow">Обновления</span>
          <CardTitle>Версии и обновления</CardTitle>
        </div>
        <button
          class="update-orbit"
          :class="{ loading: checkingAllUpdates }"
          type="button"
          aria-label="Проверить обновления приложения и flowseal-zapret"
          title="Проверить обновления приложения и flowseal-zapret"
          :disabled="checkingAllUpdates || benchRunning"
          @click="checkAllUpdates"
        >
          <RefreshCw :size="17" />
        </button>
      </CardHeader>
      <CardContent class="updates-content">
        <div class="update-row">
          <div>
            <span class="update-name">Приложение</span>
            <strong class="update-version">{{ appVersion || "—" }}</strong>
          </div>
          <div class="update-actions">
            <span v-if="appHasUpdate" class="update-available"
              >Доступно {{ appUpdate?.latest }}</span
            >
            <button
              v-if="appHasUpdate"
              class="btn small primary"
              type="button"
              :disabled="busy || benchRunning || !appUpdate?.can_auto_update"
              @click="applyAppUpdate"
            >
              Обновить
            </button>
            <button
              v-else
              class="btn small"
              type="button"
              :disabled="checkingAppUpdate || benchRunning"
              @click="checkAppUpdate(false)"
            >
              {{ checkingAppUpdate ? "Проверка…" : "Проверить" }}
            </button>
          </div>
        </div>

        <div class="update-row">
          <div>
            <span class="update-name">flowseal-zapret</span>
            <strong class="update-version">{{
              status?.installed_version || "не установлен"
            }}</strong>
          </div>
          <div class="update-actions">
            <span v-if="zapretHasUpdate" class="update-available">
              Доступно {{ updateInfo?.latest }}
            </span>
            <button
              v-if="zapretHasUpdate"
              class="btn small primary"
              type="button"
              :disabled="busy || benchRunning"
              @click="applyUpdate"
            >
              Обновить
            </button>
            <button
              v-else
              class="btn small"
              type="button"
              :disabled="checkingUpdate || benchRunning"
              @click="checkUpdate(false)"
            >
              {{ checkingUpdate ? "Проверка…" : "Проверить" }}
            </button>
          </div>
        </div>

        <div class="update-row">
          <div>
            <span class="update-name">Telegram WS-прокси</span>
            <strong class="update-version">{{ telegramVersion }}</strong>
          </div>
          <div class="update-actions">
            <span v-if="tgwsHasUpdate" class="update-available">
              Доступно {{ tgwsUpdate?.latest }}
            </span>
            <button
              v-if="tgwsHasUpdate"
              class="btn small primary"
              type="button"
              :disabled="tgwsBusy"
              @click="applyTgwsUpdate"
            >
              Обновить
            </button>
            <button
              v-else
              class="btn small"
              type="button"
              :disabled="tgwsChecking"
              @click="checkTgwsUpdate(false, true)"
            >
              {{ tgwsChecking ? "Проверка…" : "Проверить" }}
            </button>
          </div>
        </div>
      </CardContent>
    </Card>

    <Card v-if="benchSummary" class="last-check-card">
      <CardContent class="last-check-content">
        <div>
          <span class="eyebrow">Последняя проверка</span>
          <strong>{{ benchSummary.best || "Без победителя" }}</strong>
        </div>
        <div class="last-check-stats">
          <span
            >Балл <strong>{{ bestScore ?? "—" }}</strong></span
          >
          <span>{{ benchDate }}</span>
        </div>
        <button class="btn small" type="button" @click="router.push('/zapret')">
          К стратегиям
          <ArrowUpRight :size="13" />
        </button>
      </CardContent>
    </Card>
  </div>
</template>
