<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from "vue";
import MarkdownBody from "./MarkdownBody.vue";
import { dismissWhatsNew, openReleasePage, whatsNew } from "../store";

const closeButton = ref<HTMLButtonElement | null>(null);

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") dismissWhatsNew();
}

onMounted(async () => {
  window.addEventListener("keydown", onKeydown);
  await nextTick();
  closeButton.value?.focus();
});

onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <div v-if="whatsNew" class="modal-backdrop" @click.self="dismissWhatsNew">
      <div
        class="modal whats-new-modal"
        role="dialog"
        aria-modal="true"
        :aria-label="`Что нового в zapret-ui ${whatsNew.version}`"
      >
        <h3>Что нового в zapret-ui {{ whatsNew.version }}</h3>
        <MarkdownBody class="whats-new-notes" :source="whatsNew.notes" />

        <div class="modal-actions">
          <button class="btn small" @click="openReleasePage(whatsNew.url)">Открыть релиз</button>
          <button ref="closeButton" class="btn primary" @click="dismissWhatsNew">Понятно</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
