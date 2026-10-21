<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import { ArrowUpRight, FolderOpen, Gauge, RefreshCw, ShieldCheck } from "lucide-vue-next";
import StatusPanel from "../components/StatusPanel.vue";
import {
  appUpdate,
  appVersion,
  benchDate,
  benchHostCount,
  benchSummary,
  checkAppUpdate,
  checkUpdate,
  openFolder,
  status,
  updateInfo,
} from "../store";

const router = useRouter();

const bestScore = computed(() => {
  const best = benchSummary.value?.best;
  if (!best) return null;
  return benchSummary.value?.results.find((result) => result.name === best)?.score ?? null;
});

const updateCount = computed(
  () => (appUpdate.value?.available ? 1 : 0) + (updateInfo.value?.update_available ? 1 : 0),
);
</script>

<template>
  <div class="page-stack">
    <div class="overview-grid">
      <StatusPanel />

      <section class="glass-card overview-card">
        <div class="card-head">
          <div>
            <span class="eyebrow">Состояние</span>
            <h2>Обновления</h2>
          </div>
          <span class="card-icon"><RefreshCw :size="17" /></span>
        </div>

        <div class="overview-row">
          <span>Приложение</span>
          <strong>{{ appVersion || "—" }}</strong>
        </div>
        <div class="overview-row">
          <span>flowseal-zapret</span>
          <strong>{{ status?.installed_version || "не установлен" }}</strong>
        </div>
        <div class="overview-row">
          <span>Доступно</span>
          <strong :class="{ accent: updateCount > 0 }">
            {{ updateCount > 0 ? `${updateCount} обновление(я)` : "актуально" }}
          </strong>
        </div>

        <button class="btn small" type="button" @click="router.push('/settings')">
          Открыть настройки
          <ArrowUpRight :size="14" />
        </button>
      </section>
    </div>

    <section class="glass-card bench-card">
      <div class="card-head">
        <div>
          <span class="eyebrow">Диагностика</span>
          <h2>Последняя проверка</h2>
        </div>
        <span class="card-icon"><Gauge :size="17" /></span>
      </div>

      <template v-if="benchSummary">
        <div class="bench-summary">
          <div class="bench-stat">
            <span>Лучший результат</span>
            <strong>{{ benchSummary.best || "—" }}</strong>
          </div>
          <div class="bench-stat">
            <span>Балл</span>
            <strong>{{ bestScore ?? "—" }}</strong>
          </div>
          <div class="bench-stat">
            <span>Проверено</span>
            <strong>{{ benchHostCount }} целей</strong>
          </div>
          <div class="bench-stat">
            <span>Когда</span>
            <strong>{{ benchDate }}</strong>
          </div>
        </div>
        <button class="btn small" type="button" @click="router.push('/zapret')">
          <ShieldCheck :size="14" />
          К стратегиям
        </button>
      </template>
      <p v-else class="muted">
        Проверка ещё не запускалась. Перейдите в Zapret и запустите «Все стратегии».
      </p>
    </section>

    <section class="glass-card quick-card">
      <div class="card-head">
        <div>
          <span class="eyebrow">Быстрый доступ</span>
          <h2>Действия</h2>
        </div>
      </div>
      <div class="quick-actions">
        <button class="btn small" type="button" @click="checkAppUpdate(false)">
          <RefreshCw :size="14" />
          Проверить приложение
        </button>
        <button class="btn small" type="button" @click="checkUpdate(false)">
          <ShieldCheck :size="14" />
          Проверить flowseal-zapret
        </button>
        <button class="btn small" type="button" @click="openFolder">
          <FolderOpen :size="14" />
          Открыть папку
        </button>
      </div>
    </section>
  </div>
</template>
