<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Bell, Minus, Square, X } from "lucide-vue-next";
import { appUpdate, statusText, statusTone, updateInfo } from "../store";

const route = useRoute();
const router = useRouter();

const title = computed(() => (route.meta.title as string | undefined) ?? "zapret-ui");
const hasUpdate = computed(
  () => !!appUpdate.value?.available || !!updateInfo.value?.update_available,
);

const appWindow = getCurrentWindow();

function minimize() {
  void appWindow.minimize();
}

function toggleMaximize() {
  void appWindow.toggleMaximize();
}

function closeWindow() {
  void appWindow.close();
}
</script>

<template>
  <header class="topbar">
    <div class="topbar-title" data-tauri-drag-region @dblclick="toggleMaximize">
      <h1 data-tauri-drag-region>{{ title }}</h1>
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

      <div class="window-controls" aria-label="Управление окном">
        <button class="window-control" type="button" aria-label="Свернуть" @click="minimize">
          <Minus :size="15" />
        </button>
        <button
          class="window-control"
          type="button"
          aria-label="Развернуть"
          @click="toggleMaximize"
        >
          <Square :size="13" />
        </button>
        <button
          class="window-control close"
          type="button"
          aria-label="Закрыть"
          @click="closeWindow"
        >
          <X :size="15" />
        </button>
      </div>
    </div>
  </header>
</template>
