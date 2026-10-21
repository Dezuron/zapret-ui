<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import StatusPanel from "./components/StatusPanel.vue";
import StrategyPanel from "./components/StrategyPanel.vue";
import MaintenancePanel from "./components/MaintenancePanel.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ToastHost from "./components/ToastHost.vue";
import SetupWizard from "./components/SetupWizard.vue";
import WhatsNewDialog from "./components/WhatsNewDialog.vue";
import { disposeStore, initStore, showSetupWizard, showWhatsNew } from "./store";

onMounted(initStore);
onUnmounted(disposeStore);
</script>

<template>
  <div class="app">
    <main class="workspace">
      <div class="column column-side">
        <StatusPanel />
        <MaintenancePanel />
      </div>

      <StrategyPanel class="column column-main" />
    </main>

    <SetupWizard v-if="showSetupWizard" />
    <WhatsNewDialog v-if="showWhatsNew" />
    <ToastHost />
    <ConfirmDialog />
  </div>
</template>
