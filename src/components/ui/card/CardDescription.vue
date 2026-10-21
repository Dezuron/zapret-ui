<script setup lang="ts">
import { cn } from "@/lib/utils";
const props = defineProps<{ class?: string }>();
</script>

<template>
  <p :class="cn('text-[11px] text-muted-foreground', props.class)"><slot /></p>
</template>
