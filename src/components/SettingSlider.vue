<script setup lang="ts">
import { ref, watch } from "vue";
import { NSlider } from "naive-ui";

/**
 * 带本地即时反馈的滑条
 * 关键点：naive-ui 的 NSlider 只发 update:value / dragend，没有 change 事件，
 * 所以这里用「拖动中更新本地值 + 200ms 防抖提交 + dragend 立即提交」。
 */
const props = withDefaults(
  defineProps<{
    modelValue: number;
    label: string;
    desc?: string;
    min: number;
    max: number;
    step?: number;
    suffix?: string;
  }>(),
  { step: 1, suffix: "", desc: "" }
);

const emit = defineEmits<{ (e: "update:modelValue", v: number): void }>();

const local = ref(props.modelValue);
let timer: number | undefined;

watch(
  () => props.modelValue,
  (v) => {
    if (v !== local.value) local.value = v;
  }
);

function onInput(v: number) {
  local.value = v;
  if (timer) clearTimeout(timer);
  timer = window.setTimeout(() => emit("update:modelValue", local.value), 200);
}

function commit() {
  if (timer) clearTimeout(timer);
  emit("update:modelValue", local.value);
}
</script>

<template>
  <div class="srow">
    <div class="rlabel">
      <p class="rt">{{ label }}</p>
      <p v-if="desc" class="rd">{{ desc }}</p>
    </div>
    <div class="sctl">
      <NSlider
        :value="local"
        :min="min"
        :max="max"
        :step="step"
        :format-tooltip="(v: number) => v + suffix"
        @update:value="onInput"
        @dragend="commit"
      />
      <span class="sval">{{ local }}{{ suffix }}</span>
    </div>
  </div>
</template>

<style scoped>
.srow {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 0;
}

.rlabel .rt {
  margin: 0 0 2px;
  font-size: 13.5px;
  color: var(--text);
}

.rlabel .rd {
  margin: 0;
  font-size: 11.5px;
  color: var(--text-muted);
}

.sctl {
  display: flex;
  align-items: center;
  gap: 12px;
}

.sctl :deep(.n-slider) {
  flex: 1;
  max-width: 340px;
}

.sval {
  min-width: 52px;
  text-align: right;
  font-size: 12px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
</style>
