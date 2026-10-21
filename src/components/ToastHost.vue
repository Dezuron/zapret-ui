<script setup lang="ts">
import { dismissToast, toasts } from "../lib/toast";
</script>

<template>
  <Teleport to="body">
    <div class="toast-host" aria-live="polite">
      <div
        v-for="item in toasts"
        :key="item.id"
        class="toast"
        :class="item.kind"
        @click="dismissToast(item.id)"
      >
        <span class="toast-text">{{ item.text }}</span>
        <button class="toast-close" aria-label="Закрыть уведомление">×</button>
      </div>
    </div>
  </Teleport>
</template>
