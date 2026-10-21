<script setup lang="ts">
defineProps<{
  step?: number | string;
  title: string;
}>();
</script>

<template>
  <section class="panel">
    <header class="panel-head">
      <span v-if="step !== undefined && step !== ''" class="step" aria-hidden="true">{{
        step
      }}</span>
      <h2>{{ title }}</h2>
      <div class="panel-actions">
        <slot name="actions" />
      </div>
    </header>
    <div class="panel-body">
      <slot />
    </div>
  </section>
</template>
