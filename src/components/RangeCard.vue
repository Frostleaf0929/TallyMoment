<script setup lang="ts">
import { computed } from "vue";

/** 时间范围卡片（详细 · 应用 与 详细 · 事项 共用同一种样式）
 *  只做"相对区间"选择：今天 / 本周 / 本月 / 本年 / 全部时间 */
export type RangeKey = "day" | "week" | "month" | "year" | "all";

const props = defineProps<{ modelValue: RangeKey; from?: string; to?: string; note?: string }>();
const emit = defineEmits<{ (e: "update:modelValue", v: RangeKey): void }>();

const ranges: { key: RangeKey; label: string; text: string }[] = [
  { key: "day", label: "天", text: "今天" },
  { key: "week", label: "周", text: "本周" },
  { key: "month", label: "月", text: "本月" },
  { key: "year", label: "年", text: "本年" },
  { key: "all", label: "总共", text: "全部时间" },
];

const text = computed(() => ranges.find((r) => r.key === props.modelValue)?.text ?? "");
</script>

<template>
  <div class="rangerow">
    <div class="rangehead">
      <h2>时间范围</h2>
      <span class="rangetext">{{ text }}</span>
    </div>
    <div class="rangeopts">
      <button
        v-for="r in ranges"
        :key="r.key"
        class="rangeopt"
        :class="{ active: modelValue === r.key }"
        @click="emit('update:modelValue', r.key)"
      >
        {{ r.label }}
      </button>
    </div>
    <p v-if="from && to" class="spantext">{{ from }} ~ {{ to }}</p>
    <p v-if="note" class="note">{{ note }}</p>
  </div>
</template>

<style scoped>
.rangerow {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.rangehead {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.rangehead h2 {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

.rangetext {
  font-size: 15px;
  font-weight: 700;
  color: var(--text);
}

.rangeopts {
  display: inline-flex;
  gap: 8px;
}

.rangeopt {
  border: 0;
  background: transparent;
  color: var(--text-faint);
  font-size: 13px;
  font-family: inherit;
  padding: 2px 8px;
  border-radius: var(--r-sm);
  cursor: pointer;
  transition: color var(--dur), background var(--dur);
}

.rangeopt:hover {
  color: var(--text);
  background: var(--surface-hover);
}

.rangeopt.active {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.spantext {
  margin: 0;
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

.note {
  margin: 2px 0 0;
  font-size: 11px;
  color: var(--accent-text);
}
</style>
