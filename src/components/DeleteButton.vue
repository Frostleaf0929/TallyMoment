<script setup lang="ts">
/**
 * 删除按钮：圆形图标 → 悬停横向展开成胶囊（胶囊内是"图标 + 中文"横向排列）
 * 动效参考 uiverse.io/vinodjangid07/smart-emu-83，但内容排布改横向
 */
withDefaults(defineProps<{ label?: string }>(), { label: "删除" });
const emit = defineEmits<{ (e: "click"): void }>();
</script>

<template>
  <button class="del" :title="label" @click="emit('click')">
    <span class="inner">
      <svg viewBox="0 0 448 512" class="del-icon" aria-hidden="true">
        <path
          d="M135.2 17.7L128 32H32C14.3 32 0 46.3 0 64S14.3 96 32 96H416c17.7 0 32-14.3 32-32s-14.3-32-32-32H320l-7.2-14.3C307.4 6.8 296.3 0 284.2 0H163.8c-12.1 0-23.2 6.8-28.6 17.7zM416 128H32L53.2 467c1.6 25.3 22.6 45 47.9 45H346.9c25.3 0 46.3-19.7 47.9-45L416 128z"
        />
      </svg>
      <span class="txt">{{ label }}</span>
    </span>
  </button>
</template>

<style scoped>
.del {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: var(--r-full);
  background: var(--surface);
  color: var(--danger);
  cursor: pointer;
  overflow: hidden;
  font-family: inherit;
  transition: width var(--dur) cubic-bezier(0.22, 0.61, 0.36, 1),
    background var(--dur), color var(--dur), border-color var(--dur);
}

.inner {
  display: inline-flex;
  align-items: center;
  gap: 0;
  white-space: nowrap;
}

.del-icon {
  width: 11px;
  height: 11px;
  flex: none;
}

.del-icon path {
  fill: currentColor;
}

.txt {
  max-width: 0;
  overflow: hidden;
  font-size: 11.5px;
  line-height: 1;
  opacity: 0;
  transition: max-width var(--dur), opacity var(--dur), margin-left var(--dur);
}

.del:hover {
  width: 74px;
  background: var(--danger);
  color: #fff;
  border-color: transparent;
}

.del:hover .txt {
  max-width: 40px;
  opacity: 1;
  margin-left: 5px;
}
</style>
