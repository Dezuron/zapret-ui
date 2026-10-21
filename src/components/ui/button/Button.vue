<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/utils";
import { buttonVariants, type ButtonVariants } from ".";

defineOptions({ inheritAttrs: false });

interface Props {
  variant?: ButtonVariants["variant"];
  size?: ButtonVariants["size"];
  class?: string;
  disabled?: boolean;
  type?: "button" | "submit" | "reset";
}

const props = withDefaults(defineProps<Props>(), {
  variant: "default",
  size: "default",
  type: "button",
  disabled: false,
});

const textColor = computed(() => {
  if (props.variant === "destructive") return "#ffffff";
  if (props.variant === "default") return "var(--primary-foreground)";
  if (props.variant === "ghost") return "var(--muted-foreground)";
  return "var(--foreground)";
});
</script>

<template>
  <button
    v-bind="$attrs"
    :type="props.type"
    :disabled="props.disabled"
    :class="cn(buttonVariants({ variant: props.variant, size: props.size }), props.class)"
  >
    <span
      :style="{ color: textColor, display: 'inline-flex', alignItems: 'center', gap: 'inherit' }"
    >
      <slot />
    </span>
  </button>
</template>
