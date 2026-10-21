<script setup lang="ts">
import Panel from "./Panel.vue";
import Toggle from "./Toggle.vue";
import MarkdownBody from "./MarkdownBody.vue";
import { formatBytes } from "../lib/labels";
import {
  applyAppUpdate,
  applyUpdate,
  appUpdate,
  appUpdateProgress,
  appVersion,
  benchRunning,
  busy,
  checkAppUpdate,
  checkUpdate,
  checkingAppUpdate,
  checkingUpdate,
  chooseRuntimeFolder,
  config,
  openFolder,
  openReleasePage,
  runtimePath,
  setAutoApply,
  setCheckAppUpdates,
  setGameFilter,
  status,
  toggleAutostart,
  toggleCheckUpdates,
  toggleFlashscore,
  updateInfo,
  updateProgress,
} from "../store";
</script>

<template>
  <Panel step="3" title="Обслуживание">
    <section class="section">
      <h3>Обновления</h3>

      <h4>Приложение</h4>
      <div class="version-row">
        <div class="version-box">
          <span class="field-label">Текущая</span>
          <span class="field-value">{{ appVersion || "—" }}</span>
        </div>
        <span class="arrow" aria-hidden="true">→</span>
        <div class="version-box" :class="{ available: appUpdate?.available }">
          <span class="field-label">Доступно</span>
          <span class="field-value">
            {{ appUpdate?.available ? appUpdate.latest : appVersion || "—" }}
          </span>
        </div>
      </div>

      <template v-if="appUpdate?.available">
        <MarkdownBody v-if="appUpdate.notes" class="changelog" :source="appUpdate.notes" />
        <button
          class="btn primary"
          :disabled="busy || benchRunning || !appUpdate.can_auto_update"
          @click="applyAppUpdate"
        >
          Обновить приложение
        </button>
        <button class="btn small" @click="openReleasePage(appUpdate.url)">
          Открыть страницу релиза
        </button>
      </template>
      <button
        v-else
        class="btn"
        :disabled="checkingAppUpdate || benchRunning"
        @click="checkAppUpdate(false)"
      >
        {{ checkingAppUpdate ? "Проверка…" : "Проверить обновление приложения" }}
      </button>
      <div v-if="appUpdateProgress" class="muted">{{ appUpdateProgress }}</div>

      <h4>flowseal-zapret</h4>
      <div class="version-row">
        <div class="version-box">
          <span class="field-label">Версия</span>
          <span class="field-value">{{ status?.installed_version || "не установлен" }}</span>
        </div>
        <span class="arrow" aria-hidden="true">→</span>
        <div class="version-box" :class="{ available: updateInfo?.update_available }">
          <span class="field-label">Доступно</span>
          <span class="field-value">
            {{
              updateInfo?.update_available ? updateInfo.latest : status?.installed_version || "—"
            }}
          </span>
        </div>
      </div>

      <template v-if="updateInfo?.update_available">
        <MarkdownBody class="changelog" :source="updateInfo.changelog" />
        <button class="btn primary" :disabled="busy || benchRunning" @click="applyUpdate">
          Обновить{{
            updateInfo.download_size ? ` (${formatBytes(updateInfo.download_size)})` : ""
          }}
        </button>
      </template>
      <button class="btn" :disabled="checkingUpdate || benchRunning" @click="checkUpdate(false)">
        {{ checkingUpdate ? "Проверка…" : "Проверить обновления flowseal-zapret" }}
      </button>
      <div v-if="updateProgress" class="muted">{{ updateProgress }}</div>
    </section>

    <section class="section">
      <h3>Настройки</h3>

      <Toggle
        label="Запуск при старте Windows"
        hint="Тихо при входе в систему"
        :checked="config?.autostart"
        :disabled="busy"
        @change="toggleAutostart"
      />
      <Toggle
        label="Проверять обновления приложения"
        hint="Проверка GitHub при запуске"
        :checked="config?.check_app_updates"
        @change="setCheckAppUpdates"
      />
      <Toggle
        label="Проверять обновления flowseal-zapret"
        hint="Проверка релизов Flowseal/zapret-discord-youtube при запуске"
        :checked="config?.check_updates"
        @change="toggleCheckUpdates"
      />
      <Toggle
        label="Обход Flashscore"
        hint="Домены Flashscore вместе с выбранной стратегией"
        :checked="config?.flashscore_bypass"
        :disabled="busy"
        @change="toggleFlashscore"
      />
      <Toggle
        label="Применять лучшую после проверки"
        hint="Автоматически включать стратегию с баллом 90 и выше"
        :checked="config?.bench_auto_apply"
        @change="setAutoApply"
      />

      <label class="field">
        <span class="field-label">Игровой фильтр</span>
        <select
          class="select"
          :value="config?.game_filter ?? 'disabled'"
          @change="setGameFilter(($event.target as HTMLSelectElement).value)"
        >
          <option value="disabled">Отключён</option>
          <option value="all">TCP + UDP</option>
          <option value="tcp">Только TCP</option>
          <option value="udp">Только UDP</option>
        </select>
      </label>

      <div class="field">
        <span class="field-label">Папка flowseal-zapret</span>
        <span class="field-value path-value" :title="runtimePath">{{ runtimePath }}</span>
        <button
          class="btn small"
          title="Сменить папку; при запущенном обходе потребуется остановка"
          @click="chooseRuntimeFolder"
        >
          Изменить
        </button>
      </div>

      <button
        class="btn small"
        title="Открыть папку flowseal-zapret: стратегии .bat, списки и bin"
        @click="openFolder"
      >
        Открыть папку flowseal-zapret
      </button>
    </section>
  </Panel>
</template>
