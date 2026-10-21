<script setup lang="ts">
defineProps<{
  label: string;
  hint?: string;
  checked?: boolean;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  change: [value: boolean];
}>();
</script>

<template>
  <label class="toggle-row" :class="{ disabled }">
    <span class="toggle-text">
      <span class="toggle-label">{{ label }}</span>
      <span v-if="hint" class="toggle-hint">{{ hint }}</span>
    </span>
    <input
      type="checkbox"
      :checked="checked"
      :disabled="disabled"
      @change="emit('change', ($event.target as HTMLInputElement).checked)"
    />
  </label>
</template>
