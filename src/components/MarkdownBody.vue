<script setup lang="ts">
import { computed } from "vue";
import { renderMarkdown } from "../lib/markdown";
import { openReleasePage } from "../store";

const props = defineProps<{ source: string }>();

const html = computed(() => renderMarkdown(props.source));

function onLinkClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  const link = target?.closest("a");
  if (!link) return;

  event.preventDefault();
  const href = link.getAttribute("href");
  if (href?.startsWith("https://")) {
    void openReleasePage(href);
  }
}
</script>

<template>
  <div class="markdown-body" @click="onLinkClick" v-html="html" />
</template>
