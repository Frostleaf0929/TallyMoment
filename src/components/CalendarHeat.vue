<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { RangeReport } from "../types";
import { fmtDuration } from "../lib/format";

/**
 * 月历热力图（对标 Catrace 的日历视图）：按天显示使用强度
 * 数据来自 range_report（区间 ≤62 天时后端按天分桶）
 */
const props = withDefaults(defineProps<{ selected?: string; compact?: boolean }>(), {
  compact: false,
});
const emit = defineEmits<{ (e: "pick", date: string): void }>();

const cursor = ref(new Date());
const map = ref<Record<string, number>>({});
const max = computed(() => Math.max(1, ...Object.values(map.value)));

function ymd(d: Date): string {
  return `${d.getFullYear()}-${`${d.getMonth() + 1}`.padStart(2, "0")}-${`${d.getDate()}`.padStart(2, "0")}`;
}

const monthLabel = computed(() => `${cursor.value.getFullYear()} 年 ${cursor.value.getMonth() + 1} 月`);

/** 6×7 网格（周一起） */
const cells = computed(() => {
  const first = new Date(cursor.value.getFullYear(), cursor.value.getMonth(), 1);
  const start = new Date(first);
  start.setDate(1 - ((first.getDay() + 6) % 7));
  const out: { date: string; day: number; inMonth: boolean; seconds: number }[] = [];
  for (let i = 0; i < 42; i++) {
    const d = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
    const key = ymd(d);
    out.push({
      date: key,
      day: d.getDate(),
      inMonth: d.getMonth() === cursor.value.getMonth(),
      seconds: map.value[key] ?? 0,
    });
  }
  return out;
});

async function load() {
  const first = new Date(cursor.value.getFullYear(), cursor.value.getMonth(), 1);
  const last = new Date(cursor.value.getFullYear(), cursor.value.getMonth() + 1, 0);
  try {
    const r = await invoke<RangeReport>("range_report", {
      from: ymd(first),
      to: ymd(last),
      appName: null,
    });
    const m: Record<string, number> = {};
    for (const b of r.buckets) {
      // 桶的 label 是 "MM/DD"，用 date 字段更稳
      m[b.date] = b.seconds;
    }
    map.value = m;
  } catch {
    map.value = {};
  }
}

/** 回到本月并选中今天 */
function goToday() {
  cursor.value = new Date();
  void load();
  emit("pick", ymd(new Date()));
}

/** 滚轮换月（60ms 防抖，避免一次滚动跳好几个月） */
let wheelLock = 0;
function onWheel(e: WheelEvent) {
  const now = Date.now();
  if (now - wheelLock < 60) return;
  wheelLock = now;
  shift(e.deltaY > 0 ? 1 : -1);
}

function shift(delta: number) {
  cursor.value = new Date(cursor.value.getFullYear(), cursor.value.getMonth() + delta, 1);
  void load();
}

const today = ymd(new Date());
const alpha = (s: number) => (s <= 0 ? 0 : 0.18 + 0.62 * Math.min(1, s / max.value));

onMounted(load);
</script>

<template>
  <div class="cal" :class="{ compact: props.compact }" @wheel.prevent="onWheel">
    <div class="head">
      <button class="nav" title="上个月" @click="shift(-1)">‹</button>
      <span class="label">{{ monthLabel }}</span>
      <button class="nav" title="下个月" @click="shift(1)">›</button>
      <!-- 定位：回到本月并选中今天（悬停时胶囊左右展开，与删除按钮同一套动效） -->
      <button class="locate" title="回到本月今天" @click="goToday">
        <svg viewBox="0 0 24 24" class="licon" aria-hidden="true">
          <circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="1.6" />
          <circle cx="12" cy="12" r="2.6" fill="currentColor" />
          <path d="M12 2.6v2.6M12 18.8v2.6M2.6 12h2.6M18.8 12h2.6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        </svg>
        <span class="ltxt">本月</span>
      </button>
    </div>
    <div class="week">
      <span v-for="w in ['一', '二', '三', '四', '五', '六', '日']" :key="w">{{ w }}</span>
    </div>
    <div class="grid">
      <button
        v-for="c in cells"
        :key="c.date"
        class="cell"
        :class="{ dim: !c.inMonth, today: c.date === today, sel: c.date === props.selected }"
        :title="`${c.date} · ${c.seconds > 0 ? fmtDuration(c.seconds) : '无记录'}`"
        @click="emit('pick', c.date)"
      >
        <span class="d">{{ c.day }}</span>
        <span
          v-if="c.seconds > 0"
          class="fill"
          :style="{ opacity: alpha(c.seconds) }"
        ></span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.cal {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* 紧凑版：整体限宽 + 格子压扁（用于和右侧卡片并排） */
.cal.compact {
  max-width: 470px;
}

.cal.compact .cell {
  aspect-ratio: 1.55;
}

.head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.label {
  font-size: 13px;
  color: var(--text);
  font-weight: 600;
}

.nav {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-sm);
  width: 22px;
  height: 22px;
  cursor: pointer;
  font-family: inherit;
  line-height: 1;
}

.nav:hover {
  color: var(--text);
  background: var(--surface-hover);
}

.wheel {
  height: 0;
}

/* 定位按钮：默认圆形图标，悬停横向展开成胶囊并浮出文字 */
.locate {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: var(--r-full);
  background: var(--surface);
  color: var(--text-muted);
  cursor: pointer;
  overflow: hidden;
  transition: width var(--dur) cubic-bezier(0.22, 0.61, 0.36, 1),
    background var(--dur), color var(--dur), border-color var(--dur);
}

.licon {
  width: 13px;
  height: 13px;
  flex: none;
}

.ltxt {
  max-width: 0;
  overflow: hidden;
  font-size: 11px;
  line-height: 1;
  opacity: 0;
  transition: max-width var(--dur), opacity var(--dur), margin-left var(--dur);
}

.locate:hover {
  width: 64px;
  color: var(--accent-text);
  border-color: var(--accent-border);
  background: var(--accent-soft);
}

.locate:hover .ltxt {
  max-width: 32px;
  opacity: 1;
  margin-left: 4px;
}

.week {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 4px;
  font-size: 10px;
  color: var(--text-faint);
  text-align: center;
}

.grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 4px;
}

.cell {
  position: relative;
  aspect-ratio: 1;
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-sm);
  cursor: pointer;
  font-family: inherit;
  font-size: 11px;
  color: var(--text-muted);
  overflow: hidden;
  transition: transform var(--dur), border-color var(--dur);
}

.cell:hover {
  transform: translateY(calc(-1px * var(--motion)));
  border-color: var(--border-strong);
}

.cell.dim {
  opacity: 0.35;
}

.cell.today {
  border-color: var(--accent-border);
}

.cell.sel {
  box-shadow: 0 0 0 2px var(--accent-soft) inset;
  color: var(--accent-text);
}

.d {
  position: relative;
  z-index: 1;
}

.fill {
  position: absolute;
  inset: 0;
  background: var(--accent);
}
</style>
