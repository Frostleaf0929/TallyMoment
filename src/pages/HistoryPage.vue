<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NDatePicker } from "naive-ui";
import type { DailyTotal, DayReport, PeriodIndex, PeriodReport } from "../types";
import { fmtDuration } from "../lib/format";
import { jumpTo, navIntent } from "../lib/uiState";
import AppRanking from "../components/AppRanking.vue";
import BallLoader from "../components/BallLoader.vue";
import DayBars from "../components/DayBars.vue";
import HourlyChart from "../components/HourlyChart.vue";

/** 历史：近 14 天 / 按月 / 按年 / 总计（单应用细查在「详细」页） */
type Mode = "recent" | "month" | "year" | "all";
const mode = ref<Mode>("recent");

const days = ref<DailyTotal[]>([]);
const selected = ref("");
const report = ref<DayReport | null>(null);

const index = ref<PeriodIndex>({ months: [], years: [] });
const selectedMonth = ref("");
const selectedYear = ref("");
const monthTs = ref(Date.now());
const yearTs = ref(Date.now());
const period = ref<PeriodReport | null>(null);
const loading = ref(false);

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
    const raw = await invoke<DailyTotal[]>("recent_daily", { days: 14 });
    // 今天在最左、越往右越早
    days.value = [...raw].reverse();
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

/** 日历选择器（与详细页统一风格） */
function onMonthPick(ts: number | null) {
  if (!ts) return;
  const d = new Date(ts);
  selectedMonth.value = `${d.getFullYear()}-${`${d.getMonth() + 1}`.padStart(2, "0")}`;
  void loadPeriod("month", selectedMonth.value);
}

function onYearPick(ts: number | null) {
  if (!ts) return;
  selectedYear.value = String(new Date(ts).getFullYear());
  void loadPeriod("year", selectedYear.value);
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

/** 卡片右上角的平均值标注 */
const recentAvg = computed(() => {
  const arr = days.value.filter((d) => d.seconds > 0);
  if (!arr.length) return "—";
  const avg = arr.reduce((s, d) => s + d.seconds, 0) / arr.length;
  return `${fmtDuration(Math.round(avg))}/天`;
});

const periodAvg = computed(() => {
  const p = period.value;
  if (!p || !p.activeDays) return "—";
  return `${fmtDuration(Math.round(p.totalSeconds / p.activeDays))}/天`;
});

const recentTrend = computed(() => ({
  labels: days.value.map((d) => d.date.slice(5).replace("-", "/")),
  series: [
    {
      name: "使用时长",
      color: "var(--accent)",
      data: days.value.map((d) => Math.round(d.seconds / 60)),
    },
  ],
}));

const periodTrend = computed(() => {
  const b = period.value?.buckets ?? [];
  return {
    labels: b.map((x) => (period.value?.kind === "month" ? x.label + "日" : x.label)),
    series: [
      {
        name: "使用时长",
        color: "var(--accent)",
        data: b.map((x) => Math.round(x.seconds / 60)),
      },
    ],
    interval: b.length > 16 ? Math.ceil(b.length / 12) - 1 : 0,
  };
});

// 来自今日页的跳转（按应用的去向由外壳切到详细页）
watch(navIntent, async (n) => {
  if (!n || n.view !== "recent") return;
  await switchMode("recent");
  if (days.value.length) await pick(days.value[0].date);
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
      <span class="sub">近 14 天 / 按月 / 按年 / 总计（单应用细查见「详细」）</span>
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
    </div>

    <!-- 周期选择：所有视图的选择器都在同一行 -->
    <div class="picker">
      <template v-if="mode === 'recent'">
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
      </template>
      <template v-else-if="mode === 'month'">
        <NDatePicker
          v-model:value="monthTs"
          type="month"
          :actions="['confirm']"
          :clearable="false"
          style="width: 170px"
          @update:value="onMonthPick"
        />
      </template>
      <template v-else-if="mode === 'year'">
        <NDatePicker
          v-model:value="yearTs"
          type="year"
          :actions="['confirm']"
          :clearable="false"
          style="width: 150px"
          @update:value="onYearPick"
        />
      </template>
      <template v-else>
        <span class="hint">全部时间累计</span>
      </template>
    </div>

    <template v-if="mode === 'recent'">
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
          <div class="cardhead">
            <h2>24 小时分布</h2>
            <span class="avg">合计 {{ fmtDuration(report.totalSeconds) }}</span>
          </div>
          <HourlyChart :slices="report.hourly" />
        </section>

        <section class="glass-card wide">
          <div class="cardhead">
            <h2>近 14 天趋势</h2>
            <span class="avg">平均 {{ recentAvg }}</span>
          </div>
          <DayBars :labels="recentTrend.labels" :series="recentTrend.series" :label-interval="0" />
        </section>

        <section class="glass-card">
          <h2>应用排行</h2>
          <AppRanking :apps="report.apps" @pick="(n: string) => jumpTo('app', n)" />
        </section>
      </template>
      <BallLoader v-else-if="loading" label="加载中…" />
    </template>

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
          <div class="cardhead">
            <h2>{{ period.kind === "month" ? "当日分布" : period.kind === "year" ? "月度趋势" : "年度趋势" }}</h2>
            <span class="avg">平均 {{ periodAvg }}</span>
          </div>
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
          <AppRanking :apps="period.apps" @pick="(n: string) => jumpTo('app', n)" />
        </section>
      </template>
      <BallLoader v-else-if="loading" label="加载中…" />
      <p v-else class="empty">这个周期没有数据</p>
    </template>
  </div>
</template>

<style scoped>
.history { display: flex; flex-direction: column; gap: 14px; }
.head { display: flex; align-items: baseline; gap: 12px; }
.head h1 { margin: 0; font-size: 20px; font-weight: 700; }
.sub { font-size: 12px; color: var(--text-muted); }
.tabs { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.tab {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-sm);
  font-size: 13px;
  font-family: inherit;
  padding: 7px 16px;
  cursor: pointer;
  transition: background var(--dur), color var(--dur);
}
.tab:hover { color: var(--text); }
.tab.active { color: var(--accent-text); background: var(--accent-soft); border-color: var(--accent-border); }
.picker { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.hint { font-size: 12px; color: var(--text-faint); }
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
  transition: border-color var(--dur), background var(--dur), transform var(--dur);
}
.day:hover { border-color: var(--border-strong); transform: translateY(calc(-2px * var(--motion))); }
.day.active { border-color: var(--accent-border); background: var(--accent-soft); color: var(--accent-text); }
.dl { font-size: 12px; font-weight: 600; }
.dv { font-size: 10px; color: var(--text-faint); }
.cards { display: grid; grid-template-columns: repeat(3, 1fr); gap: 14px; }
.glass-card { padding: 16px 18px; }
.glass-card.wide { padding-bottom: 12px; }
.cardhead { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
.avg { font-size: 11px; color: var(--text-faint); font-variant-numeric: tabular-nums; }
.glass-card h2 { margin: 0 0 12px; font-size: 13px; font-weight: 600; color: var(--text-muted); }
.stat .label { margin: 0 0 8px; font-size: 12px; color: var(--text-muted); }
.stat .value { margin: 0; font-size: 22px; font-weight: 700; color: var(--text); font-variant-numeric: tabular-nums; }
.stat .value.accent { color: var(--accent-text); }
.stat .value.num2 { font-size: 14px; }
.stat small { font-size: 12px; font-weight: 400; color: var(--text-faint); }
.empty { color: var(--text-faint); font-size: 13px; }
</style>
