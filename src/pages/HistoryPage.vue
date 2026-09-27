<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { DailyTotal, DayReport, PeriodIndex, PeriodReport } from "../types";
import { fmtDuration } from "../lib/format";
import DayBars from "../components/DayBars.vue";
import HourlyChart from "../components/HourlyChart.vue";
import AppRanking from "../components/AppRanking.vue";

type Mode = "recent" | "month" | "year" | "all";
const mode = ref<Mode>("recent");

const days = ref<DailyTotal[]>([]);
const selected = ref("");
const report = ref<DayReport | null>(null);
const loading = ref(false);

const index = ref<PeriodIndex>({ months: [], years: [] });
const selectedMonth = ref("");
const selectedYear = ref("");
const period = ref<PeriodReport | null>(null);

const modes: { key: Mode; label: string }[] = [
  { key: "recent", label: "近 14 天" },
  { key: "month", label: "按月" },
  { key: "year", label: "按年" },
  { key: "all", label: "总计" },
];

function dayLabel(d: string): string {
  const today = new Date().toISOString().slice(0, 10);
  if (d === today) return "今天";
  const dt = new Date(d + "T00:00:00");
  return `${dt.getMonth() + 1}/${dt.getDate()}`;
}

async function loadDays() {
  try {
    days.value = await invoke<DailyTotal[]>("recent_daily", { days: 14 });
    if (days.value.length && !selected.value) {
      await pick(days.value[0].date);
    }
  } catch {
    /* 忽略 */
  }
}

async function pick(date: string) {
  selected.value = date;
  loading.value = true;
  try {
    report.value = await invoke<DayReport>("day_report", { date });
  } catch {
    report.value = null;
  } finally {
    loading.value = false;
  }
}

async function loadPeriod(kind: "month" | "year" | "all", key: string) {
  loading.value = true;
  try {
    period.value = await invoke<PeriodReport>("period_report", { kind, key });
  } catch {
    period.value = null;
  } finally {
    loading.value = false;
  }
}

async function switchMode(m: Mode) {
  mode.value = m;
  if (m === "recent") {
    await loadDays();
  } else if (m === "month") {
    if (!selectedMonth.value) selectedMonth.value = index.value.months[0] ?? "";
    if (selectedMonth.value) await loadPeriod("month", selectedMonth.value);
  } else if (m === "year") {
    if (!selectedYear.value) selectedYear.value = index.value.years[0] ?? "";
    if (selectedYear.value) await loadPeriod("year", selectedYear.value);
  } else {
    await loadPeriod("all", "");
  }
}

/** 近 14 天趋势：按天 */
const recentTrend = computed(() => ({
  labels: days.value.map((d) => d.date.slice(5).replace("-", "/")),
  series: [
    {
      name: "使用时长",
      color: "rgba(123,132,236,0.75)",
      data: days.value.map((d) => Math.round(d.seconds / 60)),
    },
  ],
}));

/** 周期趋势：月=按天、年=按月、总=按年 */
const periodTrend = computed(() => {
  const b = period.value?.buckets ?? [];
  return {
    labels: b.map((x) => (period.value?.kind === "month" ? x.label + "日" : x.label)),
    series: [
      {
        name: "使用时长",
        color: "rgba(123,132,236,0.75)",
        data: b.map((x) => Math.round(x.seconds / 60)),
      },
    ],
    interval: b.length > 16 ? Math.ceil(b.length / 12) - 1 : 0,
  };
});

onMounted(async () => {
  try {
    index.value = await invoke<PeriodIndex>("period_index");
  } catch {
    /* 忽略 */
  }
  await loadDays();
});
</script>

<template>
  <div class="history">
    <header class="head">
      <h1>历史</h1>
      <span class="sub">近 14 天 / 按月 / 按年 / 总计</span>
    </header>

    <div class="tabs">
      <button
        v-for="m in modes"
        :key="m.key"
        class="tab"
        :class="{ active: mode === m.key }"
        @click="switchMode(m.key)"
      >
        {{ m.label }}
      </button>

      <select v-if="mode === 'month'" v-model="selectedMonth" class="pick" @change="loadPeriod('month', selectedMonth)">
        <option v-for="m in index.months" :key="m" :value="m">{{ m }}</option>
      </select>
      <select v-if="mode === 'year'" v-model="selectedYear" class="pick" @change="loadPeriod('year', selectedYear)">
        <option v-for="y in index.years" :key="y" :value="y">{{ y }} 年</option>
      </select>
    </div>

    <!-- 近 14 天 -->
    <template v-if="mode === 'recent'">
      <div class="days">
        <button
          v-for="d in days"
          :key="d.date"
          class="day"
          :class="{ active: d.date === selected }"
          @click="pick(d.date)"
        >
          <span class="dl">{{ dayLabel(d.date) }}</span>
          <span class="dv">{{ fmtDuration(d.seconds) }}</span>
        </button>
        <p v-if="!days.length" class="empty">还没有历史数据，明天再看这里</p>
      </div>

      <template v-if="report">
        <section class="cards">
          <div class="glass-card stat">
            <p class="label">当日总时长</p>
            <p class="value accent">{{ fmtDuration(report.totalSeconds) }}</p>
          </div>
          <div class="glass-card stat">
            <p class="label">使用应用</p>
            <p class="value">{{ report.appCount }} <small>个</small></p>
          </div>
          <div class="glass-card stat">
            <p class="label">键入 / 点击</p>
            <p class="value num2">
              {{ report.keys }} <small>键</small> · {{ report.clicks }} <small>击</small>
            </p>
          </div>
        </section>

        <section class="glass-card wide">
          <h2>24 小时分布</h2>
          <HourlyChart :slices="report.hourly" />
        </section>

        <section class="glass-card wide">
          <h2>近 14 天趋势</h2>
          <DayBars :labels="recentTrend.labels" :series="recentTrend.series" />
        </section>

        <section class="glass-card">
          <h2>应用排行</h2>
          <AppRanking :apps="report.apps" />
        </section>
      </template>
      <p v-else-if="loading" class="empty">加载中…</p>
    </template>

    <!-- 按月 / 按年 / 总计 -->
    <template v-else>
      <template v-if="period">
        <section class="cards">
          <div class="glass-card stat">
            <p class="label">{{ period.title }} · 总时长</p>
            <p class="value accent">{{ fmtDuration(period.totalSeconds) }}</p>
          </div>
          <div class="glass-card stat">
            <p class="label">活跃天数 / 应用数</p>
            <p class="value">{{ period.activeDays }} <small>天</small> · {{ period.appCount }} <small>个</small></p>
          </div>
          <div class="glass-card stat">
            <p class="label">键入 / 点击</p>
            <p class="value num2">
              {{ period.keys }} <small>键</small> · {{ period.clicks }} <small>击</small>
            </p>
          </div>
        </section>

        <section class="glass-card wide">
          <h2>{{ period.kind === "month" ? "当日分布" : period.kind === "year" ? "月度趋势" : "年度趋势" }}</h2>
          <DayBars
            :labels="periodTrend.labels"
            :series="periodTrend.series"
            :label-interval="periodTrend.interval"
          />
        </section>

        <section class="glass-card wide">
          <h2>24 小时分布（整段累计）</h2>
          <HourlyChart :slices="period.hourly" />
        </section>

        <section class="glass-card">
          <h2>应用排行</h2>
          <AppRanking :apps="period.apps" />
        </section>
      </template>
      <p v-else-if="loading" class="empty">加载中…</p>
      <p v-else class="empty">这个周期没有数据</p>
    </template>
  </div>
</template>

<style scoped>
.history {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.head {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.head h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
}

.sub {
  font-size: 12px;
  color: var(--text-muted);
}

.tabs {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tab {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-sm);
  font-size: 13px;
  font-family: inherit;
  padding: 7px 16px;
  cursor: pointer;
}

.tab:hover {
  color: var(--text);
}

.tab.active {
  color: var(--accent-text);
  background: var(--accent-soft);
  border-color: var(--accent-border);
}

.pick {
  margin-left: 4px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  border-radius: var(--r-sm);
  font-size: 13px;
  font-family: inherit;
  padding: 6px 10px;
}

.days {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.day {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-md);
  padding: 8px 12px;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 76px;
  font-family: inherit;
}

.day:hover {
  border-color: var(--border-strong);
}

.day.active {
  border-color: var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent-text);
}

.dl {
  font-size: 12px;
  font-weight: 600;
}

.dv {
  font-size: 10px;
  color: var(--text-faint);
}

.cards {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 14px;
}

.glass-card {
  padding: 16px 18px;
}

.glass-card.wide {
  padding-bottom: 12px;
}

.glass-card h2 {
  margin: 0 0 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

.stat .label {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--text-muted);
}

.stat .value {
  margin: 0;
  font-size: 22px;
  font-weight: 700;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.stat .value.accent {
  color: var(--accent-text);
}

.stat .value.num2 {
  font-size: 14px;
}

.stat small {
  font-size: 12px;
  font-weight: 400;
  color: var(--text-faint);
}

.empty {
  color: var(--text-faint);
  font-size: 13px;
}
</style>
