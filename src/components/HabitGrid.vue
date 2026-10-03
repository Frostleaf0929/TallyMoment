<script setup lang="ts">
import { computed, ref } from "vue";
import type { HabitRow } from "../types";

/** 习惯追踪（参考 dsh whale 的习惯追踪器）：固定事项 × 近 N 天完成格
 *  点行名选中，点格子直接打卡（当天没安排的过去日期 = 补卡完成） */
const props = defineProps<{ rows: HabitRow[]; modelValue: number | null }>();
const emit = defineEmits<{
  (e: "select", taskId: number): void;
  (e: "toggle", taskId: number, date: string): void;
}>();

const spanDays = ref<7 | 14 | 30>(7);
const spans: { d: 7 | 14 | 30; label: string }[] = [
  { d: 7, label: "7 天" },
  { d: 14, label: "14 天" },
  { d: 30, label: "30 天" },
];
const viewMode = ref<"grid" | "timeline">("grid");

const repeatLabel = (mode: string): string =>
  ({ daily: "每天", weekly: "每周", monthly: "每月", yearly: "每年" })[mode] ?? "";

function ymd(d: Date): string {
  const m = `${d.getMonth() + 1}`.padStart(2, "0");
  const day = `${d.getDate()}`.padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

const todayStr = ymd(new Date());

/** 格点行的列定义：repeat() 的次数不能用 CSS 变量（v-bind 不行），用内联 style 生成 */
const rowGrid = computed(
  () => `minmax(130px, 1fr) repeat(${spanDays.value}, 34px)`
);

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

/** 时间轴的日期标注抽稀：30 天时隔天标注，避免标签互相重叠 */
const labelStep = computed(() => (spanDays.value > 20 ? 2 : 1));
const axisLabeled = computed(() => axis.value.filter((_, i) => i % labelStep.value === 0));

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
  if (!c) return `${date} · 未安排（点击补卡）`;
  const mins = c.spentMin != null ? `，用时 ${c.spentMin} 分钟` : "";
  return `${date} · ${c.done ? "已完成，点击取消" : "没完成，点击标记"}${mins}`;
}

/** 点格子：今天/过去的日期才允许打卡（未来还没到，不能预打卡） */
function onCell(row: HabitRow, date: string) {
  if (date > todayStr) return;
  emit("toggle", row.taskId, date);
}

/* ---------- 时间轴视图：每行一根横条（首次安排 → 最近安排），完成日画实点 ---------- */
const idxOf = computed(() => {
  const m = new Map<string, number>();
  axis.value.forEach((a, i) => m.set(a.date, i));
  return m;
});

function barStyle(row: HabitRow) {
  const dates = row.cells.map((c) => c.date).filter((d) => idxOf.value.has(d)).sort();
  if (!dates.length) return { display: "none" };
  const a = idxOf.value.get(dates[0]) ?? 0;
  const b = idxOf.value.get(dates[dates.length - 1]) ?? 0;
  const n = spanDays.value;
  return {
    left: `${(a / n) * 100}%`,
    width: `${Math.max(2, ((b - a + 1) / n) * 100)}%`,
  };
}

function dotStyle(date: string) {
  const i = idxOf.value.get(date);
  if (i == null) return { display: "none" };
  return { left: `${((i + 0.5) / spanDays.value) * 100}%` };
}
</script>

<template>
  <div>
    <div class="hg-head">
      <p class="hg-title">习惯追踪 · 固定事项完成情况（点行名选中，点格子打卡）</p>
      <div class="hg-ctl">
        <div class="hg-mode">
          <button :class="{ on: viewMode === 'grid' }" @click="viewMode = 'grid'">格点</button>
          <button :class="{ on: viewMode === 'timeline' }" @click="viewMode = 'timeline'">时间轴</button>
        </div>
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
    </div>

    <p v-if="!rows.length" class="hg-empty">
      还没有固定事项：在〈任务〉新建时把重复选成「每天 / 每周…」，它就会出现在这里，坚持情况一眼可见。
    </p>

    <!-- 格点视图 -->
    <div v-else-if="viewMode === 'grid'" class="hg-scroll">
      <div class="hg-grid">
        <div class="hg-row head" :style="{ gridTemplateColumns: rowGrid }">
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
          :style="{ gridTemplateColumns: rowGrid }"
          @click="emit('select', r.taskId)"
        >
          <span class="hg-name">
            <em v-if="repeatLabel(r.repeatMode)" class="rep">{{ repeatLabel(r.repeatMode) }}</em>
            <span class="nm">{{ r.content }}</span>
          </span>
          <span v-for="a in axis" :key="a.date" class="hg-cell" :title="tip(r, a.date)" @click.stop="onCell(r, a.date)">
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

    <!-- 时间轴视图 -->
    <div v-else class="hg-scroll">
      <div class="hg-grid tl">
        <div class="hg-row head">
          <span class="hg-name"></span>
          <span class="tl-axis">
            <i v-for="a in axisLabeled" :key="a.date" :style="{ left: `${((idxOf.get(a.date)! + 0.5) / spanDays) * 100}%` }">{{ a.md }}</i>
          </span>
        </div>
        <div
          v-for="r in rows"
          :key="r.taskId"
          class="hg-row"
          :class="{ sel: modelValue === r.taskId }"
          @click="emit('select', r.taskId)"
        >
          <span class="hg-name">
            <em v-if="repeatLabel(r.repeatMode)" class="rep">{{ repeatLabel(r.repeatMode) }}</em>
            <span class="nm">{{ r.content }}</span>
          </span>
          <div class="tl-track">
            <span class="tl-bar" :style="barStyle(r)"></span>
            <span
              v-for="c in r.cells"
              :key="c.date"
              class="tl-dot"
              :class="{ done: c.done }"
              :style="dotStyle(c.date)"
              :title="tip(r, c.date)"
              @click.stop="onCell(r, c.date)"
            ></span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.hg-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 10px;
  flex-wrap: wrap;
}

.hg-title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

.hg-ctl {
  display: flex;
  align-items: center;
  gap: 8px;
}

.hg-mode,
.hg-spans {
  display: flex;
  gap: 4px;
}

.hg-mode button,
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

.hg-mode button.on,
.hg-spans button.on {
  color: var(--accent-text);
  background: var(--accent-soft);
  border-color: var(--accent-border);
}

/* 关键：容器宽度 = 内容宽（不随可视区截断），横向滚动时行背景跟着走 */
.hg-scroll {
  overflow-x: auto;
}

.hg-grid {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: max-content;
  min-width: 100%;
}

.hg-row {
  display: grid;
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
  cursor: pointer;
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

.hg-cell:hover .dot {
  transform: scale(1.12);
}

.dot.done {
  background: var(--accent);
  border-color: var(--accent);
}

.dot.done svg {
  width: 12px;
  height: 12px;
}

/* 当天安排了但没完成：虚线圈提示 */
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

/* ---------- 时间轴视图 ---------- */
.hg-row.tl-row,
.hg-grid.tl .hg-row {
  grid-template-columns: minmax(130px, 1fr) 1fr;
  min-width: 420px;
}

.tl-axis,
.tl-track {
  position: relative;
  height: 22px;
  margin-right: 4px;
}

.tl-axis {
  height: 16px;
}

.tl-axis i {
  position: absolute;
  top: 0;
  transform: translateX(-50%);
  font-style: normal;
  font-size: 9.5px;
  color: var(--text-faint);
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.tl-track {
  background: var(--chart-rail);
  border-radius: var(--r-full);
}

.tl-bar {
  position: absolute;
  top: 5px;
  bottom: 5px;
  border-radius: var(--r-full);
  background: color-mix(in srgb, var(--accent) 22%, transparent);
}

.tl-dot {
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 10px;
  height: 10px;
  border-radius: 50%;
  border: 1.5px solid var(--border-strong);
  background: var(--surface-solid);
  cursor: pointer;
}

.tl-dot.done {
  background: var(--accent);
  border-color: var(--accent);
}

.hg-empty {
  margin: 0;
  font-size: 12.5px;
  color: var(--text-faint);
}
</style>
