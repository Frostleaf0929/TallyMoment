<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type {
  AppPeriodReport,
  AppUsage,
  DailyTotal,
  DayReport,
  PeriodIndex,
  PeriodReport,
} from "../types";
import { fmtDuration } from "../lib/format";
import { colorFor } from "../lib/colors";
import AppRanking from "../components/AppRanking.vue";
import { jumpTo, navIntent } from "../lib/uiState";
import DayBars from "../components/DayBars.vue";
import HourlyChart from "../components/HourlyChart.vue";
import BallLoader from "../components/BallLoader.vue";

type Mode = "recent" | "month" | "year" | "all" | "app";
const mode = ref<Mode>("recent");

const days = ref<DailyTotal[]>([]);
const selected = ref("");
const report = ref<DayReport | null>(null);

const index = ref<PeriodIndex>({ months: [], years: [] });
const selectedMonth = ref("");
const selectedYear = ref("");
const period = ref<PeriodReport | null>(null);

/* 按应用 */
const apps = ref<AppUsage[]>([]);
const appName = ref("");
const appKind = ref<"day" | "month" | "year" | "all">("month");
const appKey = ref("");
const appReport = ref<AppPeriodReport | null>(null);

const loading = ref(false);

const modes: { key: Mode; label: string }[] = [
  { key: "recent", label: "近 14 天" },
  { key: "month", label: "按月" },
  { key: "year", label: "按年" },
  { key: "all", label: "总计" },
  { key: "app", label: "按应用" },
];
const appKinds: { key: "day" | "month" | "year" | "all"; label: string }[] = [
  { key: "day", label: "日" },
  { key: "month", label: "月" },
  { key: "year", label: "年" },
  { key: "all", label: "总" },
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
    // 今天在最左、越往右越早（Tai 的顺序）
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

async function loadAppReport() {
  if (!appName.value) return;
  loading.value = true;
  try {
    appReport.value = await invoke<AppPeriodReport>("app_period_report", {
      name: appName.value,
      kind: appKind.value,
      key: appKind.value === "day" ? appDay.value : appKey.value,
    });
  } catch {
    appReport.value = null;
  } finally {
    loading.value = false;
  }
}

/** 按应用：当前周期的可选 key */
const appDay = ref("");
const appKeys = computed(() => {
  if (appKind.value === "day") return days.value.map((d) => d.date);
  if (appKind.value === "month") return index.value.months;
  if (appKind.value === "year") return index.value.years;
  return [];
});

function syncAppKey() {
  if (appKind.value === "day") appDay.value = appDay.value || days.value[0]?.date || "";
  else if (appKind.value === "month") appKey.value = appKey.value || index.value.months[0] || "";
  else if (appKind.value === "year") appKey.value = appKey.value || index.value.years[0] || "";
  else appKey.value = "";
}

async function switchMode(m: Mode) {
  mode.value = m;
  if (m === "recent") await loadDays();
  else if (m === "month") {
    selectedMonth.value = selectedMonth.value || index.value.months[0] || "";
    if (selectedMonth.value) await loadPeriod("month", selectedMonth.value);
  } else if (m === "year") {
    selectedYear.value = selectedYear.value || index.value.years[0] || "";
    if (selectedYear.value) await loadPeriod("year", selectedYear.value);
  } else if (m === "all") {
    await loadPeriod("all", "");
  } else {
    if (!apps.value.length) {
      try {
        apps.value = await invoke<AppUsage[]>("app_list", { limit: 100 });
      } catch {
        /* 忽略 */
      }
    }
    appName.value = appName.value || apps.value[0]?.name || "";
    syncAppKey();
    await loadAppReport();
  }
}

async function pickApp(name: string) {
  appName.value = name;
  await loadAppReport();
}

// 来自今日页/排行的跳转
watch(navIntent, async (n) => {
  if (!n) return;
  if (n.view === "recent") {
    await switchMode("recent");
    if (days.value.length) await pick(days.value[0].date);
  } else {
    if (!apps.value.length) {
      try {
        apps.value = await invoke<AppUsage[]>("app_list", { limit: 100 });
      } catch {
        /* 忽略 */
      }
    }
    await switchMode("app");
    if (n.app) await pickApp(n.app);
  }
});

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
  const avg = p.totalSeconds / p.activeDays;
  return `${fmtDuration(Math.round(avg))}/天`;
});

async function pickAppKind(k: "day" | "month" | "year" | "all") {
  appKind.value = k;
  syncAppKey();
  await loadAppReport();
}

const appColor = computed(() => colorFor(appName.value || "x"));

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

const appTrend = computed(() => {
  const b = appReport.value?.buckets ?? [];
  return {
    labels: b.map((x) => (appKind.value === "day" ? x.label + "时" : x.label)),
    series: [{ name: "使用时长", color: appColor.value, data: b.map((x) => Math.round(x.seconds / 60)) }],
    interval: b.length > 16 ? Math.ceil(b.length / 12) - 1 : 0,
  };
});

// 主题切换时对比色/色板重算（DayBars 内部已处理，这里只保证 AppRanking 重绘）
watch(mode, () => {
  /* 切视图时不做额外处理 */
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
      <span class="sub">近 14 天 / 按月 / 按年 / 总计 / 按应用</span>
    </header>

    <!-- 视图切换 -->
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

    <!-- 周期选择：所有视图的选择器都放在同一行（此前"按月"的选择器孤零零挂在上面） -->
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
        <select v-model="selectedMonth" class="sel" @change="loadPeriod('month', selectedMonth)">
          <option v-for="m in index.months" :key="m" :value="m">{{ m }}</option>
        </select>
      </template>
      <template v-else-if="mode === 'year'">
        <select v-model="selectedYear" class="sel" @change="loadPeriod('year', selectedYear)">
          <option v-for="y in index.years" :key="y" :value="y">{{ y }} 年</option>
        </select>
      </template>
      <template v-else-if="mode === 'all'">
        <span class="hint">全部时间累计</span>
      </template>
      <template v-else>
        <div class="seg">
          <button
            v-for="k in appKinds"
            :key="k.key"
            class="tab small"
            :class="{ active: appKind === k.key }"
            @click="pickAppKind(k.key)"
          >
            {{ k.label }}
          </button>
        </div>
        <select v-if="appKind === 'day'" v-model="appDay" class="sel" @change="loadAppReport">
          <option v-for="d in appKeys" :key="d" :value="d">{{ d }}</option>
        </select>
        <select v-else-if="appKind === 'month'" v-model="appKey" class="sel" @change="loadAppReport">
          <option v-for="m in appKeys" :key="m" :value="m">{{ m }}</option>
        </select>
        <select v-else-if="appKind === 'year'" v-model="appKey" class="sel" @change="loadAppReport">
          <option v-for="y in appKeys" :key="y" :value="y">{{ y }} 年</option>
        </select>
      </template>
    </div>

    <!-- 近 14 天 -->
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
          <AppRanking :apps="report.apps" @pick="(n: string) => pickApp(n)" />
        </section>
      </template>
      <BallLoader v-else-if="loading" label="加载中…" />
    </template>

    <!-- 按月 / 按年 / 总计 -->
    <template v-else-if="mode !== 'app'">
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

    <!-- 按应用 -->
    <template v-else>
      <div class="appdetail">
        <aside class="applist">
          <button
            v-for="a in apps"
            :key="a.name"
            class="approw"
            :class="{ active: a.name === appName }"
            @click="pickApp(a.name)"
          >
            <span class="adot" :style="{ background: colorFor(a.name) }"></span>
            <span class="aname" :title="a.displayName">{{ a.displayName }}</span>
            <span class="atime">{{ fmtDuration(a.seconds) }}</span>
          </button>
          <p v-if="!apps.length" class="empty">还没有应用记录</p>
        </aside>

        <div class="appmain">
          <template v-if="appReport">
            <section class="cards two">
              <div class="glass-card stat">
                <p class="label">{{ appReport.title }} · 总时长</p>
                <p class="value accent">{{ fmtDuration(appReport.totalSeconds) }}</p>
              </div>
              <div class="glass-card stat">
                <p class="label">活跃 {{ appKind === "day" ? "小时数" : "天数" }}</p>
                <p class="value">{{ appReport.activeDays }} <small>个</small></p>
              </div>
            </section>
            <section class="glass-card wide">
              <h2>
                {{
                  appKind === "day"
                    ? "当日各小时"
                    : appKind === "month"
                    ? "当月按天"
                    : appKind === "year"
                    ? "当年按月"
                    : "历年趋势"
                }}
              </h2>
              <DayBars
                :labels="appTrend.labels"
                :series="appTrend.series"
                :label-interval="appTrend.interval"
              />
            </section>
          </template>
          <BallLoader v-else-if="loading" label="加载中…" />
        </div>
      </div>
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
  flex-wrap: wrap;
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
  transition: background 0.15s, color 0.15s, transform 0.15s;
}

.tab:hover {
  color: var(--text);
}

.tab.small {
  padding: 5px 12px;
  font-size: 12px;
}

.tab.active {
  color: var(--accent-text);
  background: var(--accent-soft);
  border-color: var(--accent-border);
}

.picker {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.sel {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  border-radius: var(--r-sm);
  font-size: 13px;
  font-family: inherit;
  padding: 6px 10px;
}

.seg {
  display: inline-flex;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  padding: 3px;
  background: var(--surface);
}

.hint {
  font-size: 12px;
  color: var(--text-faint);
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
  transition: border-color 0.15s, background 0.15s;
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

.cards.two {
  grid-template-columns: repeat(2, 1fr);
}

.glass-card {
  padding: 16px 18px;
}

.glass-card.wide {
  padding-bottom: 12px;
}

.cardhead {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.avg {
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
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

/* 按应用：左侧应用栏 + 右侧详情 */
.appdetail {
  display: grid;
  grid-template-columns: 220px 1fr;
  gap: 14px;
  align-items: start;
}

.applist {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 62vh;
  overflow-y: auto;
  padding-right: 4px;
}

.approw {
  display: grid;
  grid-template-columns: 10px 1fr auto;
  align-items: center;
  gap: 8px;
  border: 1px solid transparent;
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-md);
  padding: 8px 10px;
  font-family: inherit;
  font-size: 12px;
  cursor: pointer;
  text-align: left;
  transition: background 0.15s, border-color 0.15s, transform 0.15s;
}

.approw:hover {
  background: var(--surface-hover);
  transform: translateX(2px);
}

.approw.active {
  border-color: var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent-text);
}

.adot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.aname {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.atime {
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

.appmain {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-width: 0;
}

.empty {
  color: var(--text-faint);
  font-size: 13px;
}
</style>
