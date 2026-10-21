<script setup lang="ts">
import { onMounted, onUnmounted, watch } from "vue";
import { RouterView, useRoute } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Sidebar from "./components/Sidebar.vue";
import TopBar from "./components/TopBar.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ToastHost from "./components/ToastHost.vue";
import SetupWizard from "./components/SetupWizard.vue";
import WhatsNewDialog from "./components/WhatsNewDialog.vue";
import { disposeStore, initStore, showSetupWizard, showWhatsNew } from "./store";

const route = useRoute();
const appWindow = getCurrentWindow();

type ResizeDirection =
  "East" | "North" | "NorthEast" | "NorthWest" | "South" | "SouthEast" | "SouthWest" | "West";

function startResize(direction: ResizeDirection) {
  void appWindow.startResizeDragging(direction);
}

onMounted(initStore);
onUnmounted(disposeStore);

watch(
  () => route.fullPath,
  () => {
    requestAnimationFrame(() => {
      document.querySelector<HTMLElement>(".page-content")?.scrollTo(0, 0);
    });
  },
);
</script>

<template>
  <div class="app-shell">
    <div class="resize-handle resize-n" @mousedown.prevent="startResize('North')"></div>
    <div class="resize-handle resize-s" @mousedown.prevent="startResize('South')"></div>
    <div class="resize-handle resize-e" @mousedown.prevent="startResize('East')"></div>
    <div class="resize-handle resize-w" @mousedown.prevent="startResize('West')"></div>
    <div class="resize-handle resize-ne" @mousedown.prevent="startResize('NorthEast')"></div>
    <div class="resize-handle resize-nw" @mousedown.prevent="startResize('NorthWest')"></div>
    <div class="resize-handle resize-se" @mousedown.prevent="startResize('SouthEast')"></div>
    <div class="resize-handle resize-sw" @mousedown.prevent="startResize('SouthWest')"></div>

    <Sidebar />

    <div class="app-main">
      <TopBar />
      <main class="page-content">
        <RouterView />
      </main>
    </div>

    <SetupWizard v-if="showSetupWizard" />
    <WhatsNewDialog v-if="showWhatsNew" />
    <ToastHost />
    <ConfirmDialog />
  </div>
</template>
