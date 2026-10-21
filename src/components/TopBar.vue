<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { Bell, Play, Square } from "lucide-vue-next";
import {
  appUpdate,
  benchRunning,
  busy,
  running,
  runtimeReady,
  statusText,
  statusTone,
  toggleStart,
  updateInfo,
} from "../store";

const route = useRoute();
const router = useRouter();

const title = computed(() => (route.meta.title as string | undefined) ?? "zapret-ui");
const hasUpdate = computed(
  () => !!appUpdate.value?.available || !!updateInfo.value?.update_available,
);
</script>

<template>
  <header class="topbar">
    <div class="topbar-title">
      <h1>{{ title }}</h1>
      <span class="topbar-subtitle">flowseal-zapret manager</span>
    </div>

    <div class="topbar-actions">
      <button
        v-if="hasUpdate"
        class="update-pill"
        type="button"
        title="Открыть обновления"
        @click="router.push('/settings')"
      >
        <Bell :size="14" />
        <span>Есть обновление</span>
      </button>

      <span class="status-chip" :class="statusTone">
        <span class="status-dot" :class="statusTone" aria-hidden="true"></span>
        <span>{{ statusText }}</span>
      </span>

      <button
        class="btn small topbar-toggle"
        :class="{ danger: running }"
        :disabled="busy || benchRunning || !runtimeReady"
        @click="toggleStart"
      >
        <Square v-if="running" :size="14" />
        <Play v-else :size="14" />
        <span>{{ running ? "Остановить" : "Запустить" }}</span>
      </button>
    </div>
  </header>
</template>
