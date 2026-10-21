<script setup lang="ts">
import { nextTick, onUnmounted, ref, watch } from "vue";
import { confirmState, resolveConfirm } from "../lib/confirm";

const confirmButton = ref<HTMLButtonElement | null>(null);

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") resolveConfirm(false);
}

watch(
  () => confirmState.open,
  async (open) => {
    if (open) {
      window.addEventListener("keydown", onKeydown);
      await nextTick();
      confirmButton.value?.focus();
    } else {
      window.removeEventListener("keydown", onKeydown);
    }
  },
);

onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <div v-if="confirmState.open" class="modal-backdrop" @click.self="resolveConfirm(false)">
      <div class="modal" role="dialog" aria-modal="true" :aria-label="confirmState.title">
        <h3>{{ confirmState.title }}</h3>
        <p v-if="confirmState.text" class="modal-text">{{ confirmState.text }}</p>

        <label v-if="confirmState.checkboxLabel" class="check-line modal-check">
          <input v-model="confirmState.checked" type="checkbox" />
          <span>{{ confirmState.checkboxLabel }}</span>
        </label>

        <div class="modal-actions">
          <button class="btn" @click="resolveConfirm(false)">
            {{ confirmState.cancelLabel }}
          </button>
          <button
            ref="confirmButton"
            class="btn"
            :class="confirmState.danger ? 'danger' : 'primary'"
            @click="resolveConfirm(true)"
          >
            {{ confirmState.confirmLabel }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
