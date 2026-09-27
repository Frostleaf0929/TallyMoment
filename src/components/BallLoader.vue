<script setup lang="ts">
/**
 * 弹跳小球加载动画
 * 参考 uiverse.io/alexruix/tame-fly-42（纯伪元素 + 多值 box-shadow，零依赖）
 * 取色改用主题令牌，深浅主题都能看清
 */
withDefaults(defineProps<{ label?: string }>(), { label: "加载中…" });
</script>

<template>
  <div class="loading">
    <div class="loader"></div>
    <p class="label">{{ label }}</p>
  </div>
</template>

<style scoped>
.loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 22px 0;
}

.loader {
  position: relative;
  width: 96px;
  height: 62px;
}

.loader:before {
  content: "";
  position: absolute;
  bottom: 18px;
  left: 40px;
  height: 22px;
  width: 22px;
  border-radius: 50%;
  background: var(--accent);
  animation: ball-bounce 0.5s ease-in-out infinite alternate;
}

.loader:after {
  content: "";
  position: absolute;
  right: 4px;
  top: 0;
  height: 6px;
  width: 34px;
  border-radius: 4px;
  box-shadow: 0 4px 0 var(--chart-split), -26px 38px 0 var(--chart-split),
    -52px 72px 0 var(--chart-split);
  animation: ball-step 1s ease-in-out infinite;
}

@keyframes ball-bounce {
  0% {
    transform: scale(1, 0.7);
  }
  40% {
    transform: scale(0.8, 1.2);
  }
  60% {
    transform: scale(1, 1);
  }
  100% {
    bottom: 96px;
    transform: scale(1, 0.7);
  }
}

@keyframes ball-step {
  0% {
    box-shadow: 0 8px 0 rgba(0, 0, 0, 0), 0 8px 0 var(--chart-split),
      -26px 38px 0 var(--chart-split), -52px 70px 0 var(--chart-split);
  }
  100% {
    box-shadow: 0 8px 0 var(--chart-split), -26px 38px 0 var(--chart-split),
      -52px 70px 0 var(--chart-split), -52px 70px 0 rgba(0, 0, 0, 0);
  }
}

.label {
  margin: 0;
  font-size: 12px;
  color: var(--text-faint);
}
</style>
