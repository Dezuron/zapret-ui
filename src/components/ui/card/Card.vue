<script setup lang="ts">
import { cn } from "@/lib/utils";

const props = defineProps<{ class?: string }>();
</script>

<template>
  <div
    :class="
      cn(
        'rounded-xl border border-border bg-card text-card-foreground shadow-[0_18px_50px_rgba(0,0,0,0.18),inset_0_1px_0_rgba(255,255,255,0.04)]',
        props.class,
      )
    "
  >
    <slot />
  </div>
</template>
