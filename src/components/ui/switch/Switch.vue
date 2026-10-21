<script setup lang="ts">
import { SwitchRoot, SwitchThumb } from "reka-ui";
import { cn } from "@/lib/utils";

const props = withDefaults(
  defineProps<{
    modelValue?: boolean;
    disabled?: boolean;
    class?: string;
  }>(),
  { modelValue: false, disabled: false },
);

const emit = defineEmits<{ "update:modelValue": [value: boolean] }>();
</script>

<template>
  <SwitchRoot
    :model-value="props.modelValue"
    :disabled="props.disabled"
    :class="
      cn(
        'peer inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border border-border bg-secondary p-0.5 shadow-[inset_0_1px_3px_rgba(0,0,0,0.35)] transition-colors data-[state=checked]:border-primary/50 data-[state=checked]:bg-primary disabled:cursor-not-allowed disabled:opacity-45',
        props.class,
      )
    "
    @update:model-value="emit('update:modelValue', $event)"
  >
    <SwitchThumb
      class="pointer-events-none block size-5 rounded-full bg-[linear-gradient(180deg,#fff,#d8dbe6)] shadow-[0_2px_5px_rgba(0,0,0,0.35)] transition-transform data-[state=checked]:translate-x-5"
    />
  </SwitchRoot>
</template>
