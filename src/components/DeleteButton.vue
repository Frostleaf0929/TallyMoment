<script setup lang="ts">
/**
 * 图标删除按钮：默认圆形，悬停时横向展开成胶囊并浮出"删除"文字
 * 参考 uiverse.io/vinodjangid07/smart-emu-83（纯 transition + ::before，无 keyframes）
 */
withDefaults(defineProps<{ label?: string; size?: "sm" | "md" }>(), { label: "删除", size: "md" });

const emit = defineEmits<{ (e: "click"): void }>();
</script>

<template>
  <button class="del" :class="size" :title="label" @click="emit('click')">
    <svg viewBox="0 0 448 512" class="del-icon" aria-hidden="true">
      <path
        d="M135.2 17.7L128 32H32C14.3 32 0 46.3 0 64S14.3 96 32 96H416c17.7 0 32-14.3 32-32s-14.3-32-32-32H320l-7.2-14.3C307.4 6.8 296.3 0 284.2 0H163.8c-12.1 0-23.2 6.8-28.6 17.7zM416 128H32L53.2 467c1.6 25.3 22.6 45 47.9 45H346.9c25.3 0 46.3-19.7 47.9-45L416 128z"
      />
    </svg>
  </button>
</template>

<style scoped>
.del {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 50%;
  background: var(--surface);
  color: var(--danger);
  cursor: pointer;
  overflow: hidden;
  transition: width 0.3s, border-radius 0.3s, background-color 0.3s, color 0.3s;
}

.del.md {
  width: 28px;
  height: 28px;
}

.del.sm {
  width: 24px;
  height: 24px;
}

.del-icon {
  width: 11px;
  transition: width 0.3s, transform 0.3s;
}

.del-icon path {
  fill: currentColor;
}

.del::before {
  position: absolute;
  top: 4px;
  content: "删除";
  font-size: 1px;
  color: transparent;
  transition: font-size 0.25s, transform 0.3s, color 0.25s;
}

.del:hover {
  width: 68px;
  border-radius: var(--r-full);
  background: var(--danger);
  color: #fff;
}

.del:hover .del-icon {
  width: 26px;
  transform: translateY(50%);
}

.del:hover::before {
  font-size: 11px;
  color: #fff;
  transform: translateY(-8px);
}
</style>
