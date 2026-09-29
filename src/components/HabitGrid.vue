<script setup lang="ts">
import { computed, ref } from "vue";
import type { HabitRow } from "../types";

/** 习惯追踪（参考 dsh whale 的习惯追踪器）：固定事项 × 近 N 天完成格
 *  点行名选中该事项，详细页下方显示它的统计 */
const props = defineProps<{ rows: HabitRow[]; modelValue: number | null }>();
const emit = defineEmits<{ (e: "select", taskId: number): void }>();

const spanDays = ref<7 | 14 | 30>(7);
const spans: { d: 7 | 14 | 30; label: string }[] = [
  { d: 7, label: "7 天" },
  { d: 14, label: "14 天" },
  { d: 30, label: "30 天" },
];

const repeatLabel = (mode: string): string =>
  ({ daily: "每天", weekly: "每周", monthly: "每月", yearly: "每年" })[mode] ?? "";

function ymd(d: Date): string {
  const m = `${d.getMonth() + 1}`.padStart(2, "0");
  const day = `${d.getDate()}`.padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

/** 近 N 天的日期轴（今天在最后） */
const axis = computed(() => {
  const out: { date: string; md: string; wk: string }[] = [];
  const wk = ["日", "一", "二", "三", "四", "五", "六"];
  for (let i = spanDays.value - 1; i >= 0; i--) {
    const d = new Date(Date.now() - i * 86400000);
    out.push({ date: ymd(d), md: `${d.getMonth() + 1}/${d.getDate()}`, wk: wk[d.getDay()] });
  }
  return out;
});

/** taskId -> { date -> cell }，避免每格重建映射 */
const maps = computed(() => {
  const m = new Map<number, Record<string, { done: boolean; spentMin: number | null }>>();
  for (const r of props.rows) {
    const inner: Record<string, { done: boolean; spentMin: number | null }> = {};
    for (const c of r.cells) inner[c.date] = { done: c.done, spentMin: c.spentMin };
    m.set(r.taskId, inner);
  }
  return m;
});

function cellOf(row: HabitRow, date: string) {
  return maps.value.get(row.taskId)?.[date];
}

function tip(row: HabitRow, date: string): string {
  const c = cellOf(row, date);
  if (!c) return `${date} · 未安排`;
  const mins = c.spentMin != null ? `，用时 ${c.spentMin} 分钟` : "";
  return `${date} · ${c.done ? "已完成" : "没完成"}${mins}`;
}
</script>

<template>
  <div>
    <div class="hg-head">
      <p class="hg-title">习惯追踪 · 固定事项完成格（点行名看统计）</p>
      <div class="hg-spans">
        <button
          v-for="s in spans"
          :key="s.d"
          :class="{ on: spanDays === s.d }"
          @click="spanDays = s.d"
        >
          {{ s.label }}
        </button>
      </div>
    </div>

    <div v-if="rows.length" class="hg-scroll">
      <div class="hg-grid">
        <div class="hg-row head">
          <span class="hg-name"></span>
          <span v-for="a in axis" :key="a.date" class="hg-col">
            <b>{{ a.md }}</b><i>{{ a.wk }}</i>
          </span>
        </div>
        <div
          v-for="r in rows"
          :key="r.taskId"
          class="hg-row"
          :class="{ sel: modelValue === r.taskId }"
          @click="emit('select', r.taskId)"
        >
          <span class="hg-name" :title="r.content">
            <em v-if="repeatLabel(r.repeatMode)" class="rep">{{ repeatLabel(r.repeatMode) }}</em>
            <span class="nm">{{ r.content }}</span>
          </span>
          <span v-for="a in axis" :key="a.date" class="hg-cell" :title="tip(r, a.date)">
            <span
              class="dot"
              :class="{ done: cellOf(r, a.date)?.done, miss: cellOf(r, a.date) && !cellOf(r, a.date)!.done }"
            >
              <svg v-if="cellOf(r, a.date)?.done" viewBox="0 0 12 12" aria-hidden="true">
                <path d="M2.5 6.5l2.4 2.4L9.6 4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
            </span>
          </span>
        </div>
      </div>
    </div>
    <p v-else class="hg-empty">
      还没有固定事项：在〈待办〉新建任务时把重复选成「每天 / 每周…」，它就会出现在这里，坚持情况一眼可见。
    </p>
  </div>
</template>

<style scoped>
.hg-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 10px;
}

.hg-title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

.hg-spans {
  display: flex;
  gap: 4px;
}

.hg-spans button {
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-faint);
  font-size: 11px;
  font-family: inherit;
  padding: 2px 10px;
  border-radius: var(--r-full);
  cursor: pointer;
  transition: background var(--dur), color var(--dur);
}

.hg-spans button.on {
  color: var(--accent-text);
  background: var(--accent-soft);
  border-color: var(--accent-border);
}

.hg-scroll {
  overflow-x: auto;
}

.hg-grid {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 520px;
}

.hg-row {
  display: grid;
  grid-template-columns: minmax(120px, 1fr) repeat(v-bind("spanDays"), 34px);
  align-items: center;
  gap: 2px;
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  background: var(--surface);
  cursor: pointer;
  transition: background var(--dur), border-color var(--dur);
}

.hg-row:hover {
  background: var(--surface-hover);
}

.hg-row.sel {
  border-color: var(--accent-border);
  background: var(--accent-soft);
}

.hg-row.head {
  border: 0;
  background: transparent;
  padding: 2px 10px;
  cursor: default;
}

.hg-name {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
  color: var(--text);
  overflow: hidden;
  white-space: nowrap;
}

.hg-name .nm {
  overflow: hidden;
  text-overflow: ellipsis;
}

.rep {
  flex: none;
  font-style: normal;
  font-size: 10px;
  color: var(--accent-text);
  background: var(--accent-soft);
  border-radius: var(--r-full);
  padding: 1px 8px;
}

.hg-col {
  display: flex;
  flex-direction: column;
  align-items: center;
  line-height: 1.25;
}

.hg-col b {
  font-size: 10.5px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
  font-weight: 600;
}

.hg-col i {
  font-style: normal;
  font-size: 9.5px;
  color: var(--text-faint);
}

.hg-cell {
  display: flex;
  justify-content: center;
}

.dot {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1.5px solid var(--border-strong);
  color: #fff;
  background: transparent;
  transition: background var(--dur), border-color var(--dur), transform var(--dur);
}

.dot.done {
  background: var(--accent);
  border-color: var(--accent);
}

.dot.done svg {
  width: 12px;
  height: 12px;
}

/* 当天安排了但没完成：显眼的空心圈（边框用警示色弱化版） */
.dot.miss {
  border-style: dashed;
  border-color: color-mix(in srgb, var(--danger) 45%, transparent);
}

/* 未安排的日子：一个很淡的点 */
.dot:not(.done):not(.miss) {
  border: 0;
  background: var(--chart-rail);
  width: 8px;
  height: 8px;
  margin: 6px;
  padding: 0;
}

.hg-empty {
  margin: 0;
  font-size: 12.5px;
  color: var(--text-faint);
}
</style>
