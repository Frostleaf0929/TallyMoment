<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NDatePicker, NInput } from "naive-ui";
import type { AppUsage, RangeReport } from "../types";
import { fmtDuration } from "../lib/format";
import { colorFor } from "../lib/colors";
import { accentColor } from "../lib/chartColors";
import { appColorMode, appNameEnglish } from "../lib/appearance";
import { navIntent } from "../lib/uiState";
import BallLoader from "../components/BallLoader.vue";
import DayBars from "../components/DayBars.vue";
import HourlyChart from "../components/HourlyChart.vue";

/** 详细页：按天/按周/按月/按年 + 日历选择器 + 应用列表（可下钻） */
type Mode = "day" | "week" | "month" | "year";

const mode = ref<Mode>("day");
/** 选择器里的日期（毫秒时间戳） */
const picked = ref<number>(Date.now());
const loading = ref(false);
const report = ref<RangeReport | null>(null);
const selectedApp = ref<string | null>(null);
const keyword = ref("");

const modes: { key: Mode; label: string }[] = [
  { key: "day", label: "按天" },
  { key: "week", label: "按周" },
  { key: "month", label: "按月" },
  { key: "year", label: "按年" },
];

const pickerType = computed(() => (mode.value === "year" ? "year" : mode.value === "month" ? "month" : "date"));

function ymd(d: Date): string {
  const m = `${d.getMonth() + 1}`.padStart(2, "0");
  const day = `${d.getDate()}`.padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

/** 把选择器日期换算成 [from, to] */
function rangeOf(): { from: string; to: string } {
  const d = new Date(picked.value);
  if (mode.value === "day") return { from: ymd(d), to: ymd(d) };
  if (mode.value === "week") {
    // 周一为一周开始
    const dow = (d.getDay() + 6) % 7;
    const start = new Date(d.getTime() - dow * 86400000);
    const end = new Date(start.getTime() + 6 * 86400000);
    return { from: ymd(start), to: ymd(end) };
  }
  if (mode.value === "month") {
    const start = new Date(d.getFullYear(), d.getMonth(), 1);
    const end = new Date(d.getFullYear(), d.getMonth() + 1, 0);
    return { from: ymd(start), to: ymd(end) };
  }
  return { from: `${d.getFullYear()}-01-01`, to: `${d.getFullYear()}-12-31` };
}

async function load() {
  const { from, to } = rangeOf();
  loading.value = true;
  try {
    report.value = await invoke<RangeReport>("range_report", {
      from,
      to,
      appName: selectedApp.value,
    });
  } catch {
    report.value = null;
  } finally {
    loading.value = false;
  }
}

function pickApp(name: string | null) {
  selectedApp.value = selectedApp.value === name ? null : name;
  void load();
}

const appLabel = (a: AppUsage) => (appNameEnglish.value ? a.name.replace(/\.exe$/i, "") : a.displayName);

const appColor = (a: AppUsage) =>
  appColorMode.value === "accent" ? accentColor() : colorFor(a.name);

const maxSeconds = computed(() =>
  Math.max(1, ...(report.value?.apps ?? []).map((a) => a.seconds))
);

const filteredApps = computed(() => {
  const k = keyword.value.trim().toLowerCase();
  const list = report.value?.apps ?? [];
  if (!k) return list;
  return list.filter((a) => appLabel(a).toLowerCase().includes(k) || a.name.toLowerCase().includes(k));
});

const trend = computed(() => {
  const b = report.value?.buckets ?? [];
  return {
    labels: b.map((x) => x.label),
    series: [
      {
        name: "使用时长",
        color: selectedApp.value ? accentColor() : "var(--accent)",
        data: b.map((x) => Math.round(x.seconds / 60)),
      },
    ],
    interval: b.length > 16 ? Math.ceil(b.length / 12) - 1 : 0,
  };
});

onMounted(load);

// 今日页/排行跳转过来：自动选中该应用
watch(navIntent, (n) => {
  if (!n) return;
  if (n.app) {
    selectedApp.value = n.app;
    void load();
  }
});
</script>

<template>
  <div class="detail">
    <header class="head">
      <h1>详细</h1>
      <span class="sub">按天 / 按周 / 按月 / 按年 · 单个应用可下钻</span>
    </header>

    <!-- 统一风格的周期选择器（对标 Tai：分段切换 + 日历弹层） -->
    <div class="pickerrow">
      <div class="seg">
        <button
          v-for="m in modes"
          :key="m.key"
          class="seg-item"
          :class="{ active: mode === m.key }"
          @click="((mode = m.key), load())"
        >
          {{ m.label }}
        </button>
      </div>
      <NDatePicker
        v-model:value="picked"
        :type="pickerType"
        :actions="['confirm']"
        :clearable="false"
        style="width: 190px"
        @update:value="load"
      />
      <span v-if="report" class="range">{{ report.title }} · 共 {{ report.days }} 天</span>
    </div>

    <template v-if="report">
      <section class="cards">
        <div class="glass-card stat">
          <p class="label">区间总时长</p>
          <p class="value accent">{{ fmtDuration(report.totalSeconds) }}</p>
        </div>
        <div class="glass-card stat">
          <p class="label">平均每活跃日</p>
          <p class="value">{{ fmtDuration(report.avgPerDay) }}</p>
        </div>
        <div class="glass-card stat">
          <p class="label">活跃天数 / 应用数</p>
          <p class="value">{{ report.activeDays }} <small>天</small> · {{ report.appCount }} <small>个</small></p>
        </div>
        <div class="glass-card stat">
          <p class="label">键入 / 点击</p>
          <p class="value num2">{{ report.keys }} <small>键</small> · {{ report.clicks }} <small>击</small></p>
        </div>
      </section>

      <div class="split">
        <!-- 应用列表（对标 Tai 的详细·应用：搜索 + 图标/色点 + 进度 + 时长） -->
        <aside class="glass-card applist">
          <div class="listhead">
            <h2>应用</h2>
            <NInput v-model:value="keyword" size="small" placeholder="搜索" clearable style="width: 120px" />
          </div>
          <div class="listbody">
            <button
              v-for="a in filteredApps"
              :key="a.name"
              class="approw"
              :class="{ active: selectedApp === a.name }"
              :title="appLabel(a)"
              @click="pickApp(a.name)"
            >
              <span class="adot" :style="{ background: appColor(a) }"></span>
              <span class="aname">{{ appLabel(a) }}</span>
              <span class="atime">{{ fmtDuration(a.seconds) }}</span>
              <span class="abar">
                <i :style="{ width: (a.seconds / maxSeconds) * 100 + '%', background: appColor(a) }"></i>
              </span>
            </button>
            <p v-if="!filteredApps.length" class="empty">这个区间没有记录</p>
          </div>
        </aside>

        <div class="right">
          <section class="glass-card wide">
            <div class="cardhead">
              <h2>
                {{ selectedApp ? `${report.appDisplay ?? selectedApp} · 趋势` : "使用趋势" }}
              </h2>
              <span class="avg">平均 {{ fmtDuration(report.avgPerDay) }}/天</span>
            </div>
            <DayBars :labels="trend.labels" :series="trend.series" :label-interval="trend.interval" />
          </section>

          <section class="glass-card wide">
            <h2>{{ selectedApp ? "该应用各小时分布" : "24 小时分布" }}</h2>
            <HourlyChart :slices="report.hourly" :height="200" />
          </section>
        </div>
      </div>
    </template>
    <BallLoader v-else-if="loading" label="加载中…" />
  </div>
</template>

<style scoped>
.detail {
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

.pickerrow {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.seg {
  display: inline-flex;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  padding: 4px;
  background: var(--surface);
}

.seg-item {
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 13px;
  font-family: inherit;
  padding: 6px 14px;
  border-radius: var(--r-sm);
  cursor: pointer;
  transition: background var(--dur), color var(--dur);
}

.seg-item.active {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.range {
  font-size: 12px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

.cards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
}

.split {
  display: grid;
  grid-template-columns: 268px 1fr;
  gap: 14px;
  align-items: start;
}

.right {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-width: 0;
}

.glass-card {
  padding: 14px 16px;
}

.glass-card.wide {
  padding-bottom: 12px;
}

.glass-card h2 {
  margin: 0 0 10px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
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

.stat .label {
  margin: 0 0 6px;
  font-size: 12px;
  color: var(--text-muted);
}

.stat .value {
  margin: 0;
  font-size: 20px;
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

.applist {
  display: flex;
  flex-direction: column;
  /* 撑满视口高度，不再"断在一半" */
  max-height: calc(100vh - 260px);
  min-height: 320px;
}

.listhead {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.listhead h2 {
  margin: 0;
}

.listbody {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding-right: 4px;
}

.approw {
  display: grid;
  grid-template-columns: 10px 1fr auto;
  grid-template-rows: auto auto;
  align-items: center;
  gap: 4px 8px;
  border: 1px solid transparent;
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-md);
  padding: 7px 9px;
  font-family: inherit;
  font-size: 12px;
  cursor: pointer;
  text-align: left;
  transition: background var(--dur), border-color var(--dur), transform var(--dur);
}

.approw:hover {
  background: var(--surface-hover);
  transform: translateX(calc(2px * var(--motion)));
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
  color: var(--text);
}

.approw.active .aname {
  color: var(--accent-text);
}

.atime {
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

.abar {
  grid-column: 1 / 4;
  display: block;
  height: 4px;
  border-radius: 2px;
  background: var(--chart-rail);
  overflow: hidden;
}

.abar i {
  display: block;
  height: 100%;
  border-radius: 2px;
  transition: width var(--dur) ease;
}

.empty {
  color: var(--text-faint);
  font-size: 12px;
  text-align: center;
  padding: 18px 0;
}
</style>
