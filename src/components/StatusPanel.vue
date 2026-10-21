<script setup lang="ts">
import Panel from "./Panel.vue";
import {
  applyStrategy,
  benchRunning,
  busy,
  currentName,
  installRuntime,
  running,
  runtimeReady,
  selected,
  starting,
  status,
  statusText,
  statusTone,
  toggleStart,
  updateProgress,
} from "../store";
</script>

<template>
  <Panel title="Подключение">
    <div class="status-line">
      <span class="status-dot" :class="statusTone" aria-hidden="true"></span>
      <div class="status-line-text">
        <span class="status-title">{{ statusText }}</span>
        <span class="muted">
          {{
            status?.installed_version
              ? "flowseal-zapret " + status.installed_version
              : "flowseal-zapret не установлен"
          }}
        </span>
      </div>
    </div>

    <div class="field-block">
      <span class="field-label">Активная стратегия</span>
      <span class="field-value" :title="currentName">{{ currentName || "—" }}</span>
    </div>

    <div v-if="selected && selected !== currentName" class="field-block selected-block">
      <span class="field-label">Выбрано</span>
      <div class="selected-row">
        <span class="field-value" :title="selected">{{ selected }}</span>
        <button class="btn small primary" :disabled="busy || benchRunning" @click="applyStrategy()">
          Применить
        </button>
      </div>
    </div>

    <button
      class="btn big"
      :class="running ? 'danger' : 'primary'"
      :disabled="busy || !runtimeReady || benchRunning"
      @click="toggleStart"
    >
      {{ starting ? "Подождите…" : running ? "Остановить обход" : "Запустить обход" }}
    </button>

    <div v-if="!runtimeReady" class="onboarding">
      <p class="onboarding-title">Нужен flowseal-zapret</p>
      <p class="muted">
        Около 10 МБ с GitHub: скачается, распакуется и сразу будет готов к запуску.
      </p>
      <button class="btn primary" :disabled="busy" @click="installRuntime">
        {{ busy ? "Установка…" : "Скачать и установить" }}
      </button>
      <div v-if="updateProgress" class="muted">{{ updateProgress }}</div>
    </div>

    <p v-if="benchRunning" class="hint">
      Идёт проверка стратегий — обход временно управляется автоматически.
    </p>
  </Panel>
</template>
