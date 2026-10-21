<script setup lang="ts">
import Panel from "./Panel.vue";
import StrategyRow from "./StrategyRow.vue";
import { hostStatusLabel } from "../lib/labels";
import {
  applyStrategy,
  benchBusy,
  benchDate,
  benchHostCount,
  benchProgress,
  benchRunning,
  benchSummary,
  budget,
  busy,
  cancelBench,
  deleteGenerated,
  gateMessage,
  generatedRows,
  includeGenerated,
  officialRows,
  quick,
  quickCheck,
  runBench,
  runSearch,
  runtimeReady,
  selectStrategy,
  selected,
  sortMode,
} from "../store";
</script>

<template>
  <Panel title="Стратегии" class="strategy-panel">
    <template #actions>
      <span v-if="benchSummary?.results.length" class="panel-count">
        {{ benchSummary.results.length }} проверено
      </span>
      <select v-model="sortMode" class="select select-sm" aria-label="Сортировка стратегий">
        <option value="score">По баллу</option>
        <option value="latency">По задержке</option>
        <option value="name">По имени</option>
      </select>
    </template>

    <div class="toolbar">
      <div class="tool-group">
        <span class="tool-label">Проверка</span>
        <div class="tool-row">
          <button
            class="btn small"
            :disabled="benchBusy || !runtimeReady"
            title="Проверить активную стратегию без остановки обхода"
            @click="quickCheck"
          >
            Текущая
          </button>
          <button
            class="btn small primary"
            :disabled="benchBusy || !runtimeReady"
            @click="runBench(includeGenerated)"
          >
            Все стратегии
          </button>
          <button v-if="benchRunning" class="btn small" @click="cancelBench">Отмена</button>
        </div>
      </div>

      <div class="tool-group">
        <span class="tool-label">Подбор новых</span>
        <div class="tool-row">
          <span class="muted">{{ budget }} шт.</span>
          <input
            v-model.number="budget"
            class="budget"
            type="range"
            min="20"
            max="100"
            step="10"
            :disabled="benchRunning"
            aria-label="Количество новых стратегий"
          />
          <button class="btn small" :disabled="benchBusy || !runtimeReady" @click="runSearch">
            Подобрать
          </button>
        </div>
      </div>
    </div>

    <label class="check-line">
      <input v-model="includeGenerated" type="checkbox" :disabled="benchRunning" />
      <span>Учитывать экспериментальные стратегии в проверке</span>
    </label>

    <div v-if="benchProgress" class="bench-progress">
      <div class="progress-bar">
        <div :style="{ width: benchProgress.percent + '%' }"></div>
      </div>
      <span class="muted">{{ benchProgress.message }}</span>
    </div>

    <div v-if="gateMessage" class="notice">{{ gateMessage }}</div>
    <div v-if="benchSummary?.message" class="notice">{{ benchSummary.message }}</div>
    <div v-if="benchSummary?.cancelled" class="notice muted">
      Проверка отменена — результаты частичные.
    </div>

    <div v-if="quick" class="quick-panel">
      <div class="quick-head">
        <span>{{ quick.running ? "Текущая стратегия" : "Замер без обхода" }}</span>
        <span
          class="score"
          :class="quick.running ? quick.verdict : quick.gate === 'notblocked' ? 'works' : 'broken'"
        >
          {{
            quick.running
              ? quick.score + "/100"
              : quick.gate === "notblocked"
                ? "DPI не блокирует"
                : "DPI блокирует"
          }}
        </span>
        <button
          class="icon-btn"
          aria-label="Закрыть результат"
          title="Закрыть"
          @click="quick = null"
        >
          ×
        </button>
      </div>
      <div class="chips">
        <span
          v-for="host in quick.hosts"
          :key="host.name"
          class="chip"
          :class="host.status"
          :title="`${host.name} · ${hostStatusLabel[host.status]}${host.detail ? ' · ' + host.detail : ''}`"
        >
          {{ host.name }}
        </span>
      </div>
    </div>

    <div v-if="benchSummary?.results.length" class="result-meta">
      <span>Проверено {{ benchDate }} · {{ benchHostCount }} хостов</span>
      <span class="legend">D — Discord · Y — YouTube · C — контрольные</span>
    </div>

    <div v-if="officialRows.length || generatedRows.length" class="strategy-list">
      <StrategyRow
        v-for="row in officialRows"
        :key="row.name"
        :row="row"
        :selected="selected === row.name"
        :best="row.name === benchSummary?.best"
        :disabled="busy || benchRunning"
        @select="selectStrategy(row.name)"
        @apply="applyStrategy(row.name)"
      />

      <template v-if="generatedRows.length">
        <div class="list-subhead">Экспериментальные</div>
        <StrategyRow
          v-for="row in generatedRows"
          :key="row.name"
          :row="row"
          :selected="selected === row.name"
          :best="row.name === benchSummary?.best"
          :disabled="busy || benchRunning"
          @select="selectStrategy(row.name)"
          @apply="applyStrategy(row.name)"
          @delete="deleteGenerated(row.name)"
        />
      </template>
    </div>
    <div v-else class="empty-state">
      <p v-if="!runtimeReady">Сначала установите flowseal-zapret — тогда появятся стратегии.</p>
      <p v-else>Стратегии не найдены. Обновите flowseal-zapret.</p>
    </div>
  </Panel>
</template>
