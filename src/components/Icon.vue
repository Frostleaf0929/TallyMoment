<script setup lang="ts">
// Solar Line Duotone 图标（CC BY 4.0 · 480 Design · via Iconify）
// SVG 以 raw 方式内联，颜色随 currentColor 走
import { computed } from "vue";

const icons = import.meta.glob("../assets/icons/*.svg", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const props = withDefaults(defineProps<{ name: string; size?: number }>(), {
  size: 18,
});

const svg = computed(() => {
  const raw = icons[`../assets/icons/${props.name}.svg`] ?? "";
  return raw
    .replace(/width="[^"]*"/, `width="${props.size}"`)
    .replace(/height="[^"]*"/, `height="${props.size}"`);
});
</script>

<template>
  <span class="icon" v-html="svg"></span>
</template>

<style scoped>
.icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  line-height: 0;
  flex: none;
}

.icon :deep(svg) {
  display: block;
}
</style>
