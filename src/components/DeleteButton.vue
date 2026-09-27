<script setup lang="ts">
/**
 * 删除按钮（横向：左图标 + 右中文），悬停填充危险色并有轻微放大
 * 参考 uiverse.io/vinodjangid07/smart-emu-83 的展开思路，改成横向更省空间
 */
withDefaults(defineProps<{ label?: string }>(), { label: "删除" });
const emit = defineEmits<{ (e: "click"): void }>();
</script>

<template>
  <button class="del" :title="label" @click="emit('click')">
    <svg viewBox="0 0 448 512" class="del-icon" aria-hidden="true">
      <path
        d="M135.2 17.7L128 32H32C14.3 32 0 46.3 0 64S14.3 96 32 96H416c17.7 0 32-14.3 32-32s-14.3-32-32-32H320l-7.2-14.3C307.4 6.8 296.3 0 284.2 0H163.8c-12.1 0-23.2 6.8-28.6 17.7zM416 128H32L53.2 467c1.6 25.3 22.6 45 47.9 45H346.9c25.3 0 46.3-19.7 47.9-45L416 128z"
      />
    </svg>
    <span class="txt">{{ label }}</span>
  </button>
</template>

<style scoped>
.del {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--danger);
  border-radius: var(--r-full);
  padding: 3px 10px 3px 8px;
  font-size: 11.5px;
  font-family: inherit;
  cursor: pointer;
  transition: background var(--dur), color var(--dur), border-color var(--dur),
    transform var(--dur);
}

.del-icon {
  width: 11px;
  height: 11px;
  flex: none;
}

.del-icon path {
  fill: currentColor;
}

.del:hover {
  background: var(--danger);
  color: #fff;
  border-color: transparent;
  transform: scale(calc(1 + 0.06 * var(--motion)));
}

.txt {
  line-height: 1;
}
</style>
