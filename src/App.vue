<script setup lang="ts">
import { onMounted, onUnmounted, watch } from "vue";
import { RouterView, useRoute } from "vue-router";
import Sidebar from "./components/Sidebar.vue";
import TopBar from "./components/TopBar.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ToastHost from "./components/ToastHost.vue";
import SetupWizard from "./components/SetupWizard.vue";
import WhatsNewDialog from "./components/WhatsNewDialog.vue";
import { disposeStore, initStore, showSetupWizard, showWhatsNew } from "./store";

const route = useRoute();

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
