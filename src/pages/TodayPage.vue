<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { DayReport } from "../types";
import { fmtDuration } from "../lib/format";
import Icon from "../components/Icon.vue";
import HeatTimeline from "../components/HeatTimeline.vue";
import HourlyChart from "../components/HourlyChart.vue";
import AppRanking from "../components/AppRanking.vue";

const report = ref<DayReport | null>(null);
const online = ref(false);
let timer: number | undefined;

async function refresh() {
  try {
    report.value = await invoke<DayReport>("today_report");
    online.value = true;
  } catch {
    online.value = false;
  }
}

onMounted(() => {
  refresh();
  timer = window.setInterval(refresh, 5000);
});
onUnmounted(() => clearInterval(timer));

const statusLabel = () => {
  const s = report.value?.current?.status;
  if (report.value?.paused) return { t: "已暂停", c: "muted" };
  if (!report.value?.recording) return { t: "休息中", c: "muted" };
  if (s === "fragmented") return { t: "碎片", c: "warn" };
  return { t: "专注中", c: "good" };
};
const startedAt = () => {
  const ts = report.value?.current?.startTs;
  if (!ts) return "—";
  return new Date(ts * 1000).toTimeString().slice(0, 5);
};
</script>

<template>
  <div class="today">
    <header class="head">
      <h1>今日</h1>
      <span class="date">
        {{
          new Date().toLocaleDateString("zh-CN", {
            month: "long",
            day: "numeric",
            weekday: "long",
          })
        }}
      </span>
    </header>

    <!-- 当前专注大分区 -->
    <section class="glass-card focus">
      <div class="f-left">
        <div class="f-status" :class="statusLabel().c">
          <span class="sdot"></span>{{ statusLabel().t }}
        </div>
        <p class="f-app">
          {{ report?.current?.displayName?.replace(/\.exe$/i, "") ?? "—" }}
        </p>
        <p class="f-sub">
          开始 {{ startedAt() }} · 今日第 {{ report?.blockIndex ?? 0 }} 个专注块
        </p>
      </div>
      <div class="f-mid">
        <p class="f-num">{{ Math.floor((report?.current?.seconds ?? 0) / 60) }}</p>
        <p class="f-unit">分钟 · 当前块</p>
      </div>
      <div class="f-right">
        <p class="f-kv"><span>今日累计</span><b>{{ fmtDuration(report?.totalSeconds ?? 0) }}</b></p>
        <p class="f-kv"><span>键入 / 点击</span><b>{{ report?.keys ?? 0 }} / {{ report?.clicks ?? 0 }}</b></p>
      </div>
    </section>

    <section class="cards">
      <div class="glass-card stat">
        <p class="label"><Icon name="clock" :size="14" /> 今日总时长</p>
        <p class="value accent">{{ fmtDuration(report?.totalSeconds ?? 0) }}</p>
      </div>
      <div class="glass-card stat">
        <p class="label"><Icon name="overview" :size="14" /> 使用应用</p>
        <p class="value">{{ report?.appCount ?? 0 }} <small>个</small></p>
      </div>
      <div class="glass-card stat">
        <p class="label"><Icon name="fire" :size="14" /> 键入 / 点击</p>
        <p class="value small num2">
          {{ report?.keys ?? 0 }} <small>键</small> · {{ report?.clicks ?? 0 }} <small>击</small>
        </p>
      </div>
    </section>

    <section class="glass-card wide">
      <h2>活动热力 · 全天</h2>
      <HeatTimeline :segments="report?.segments ?? []" />
    </section>

    <section class="grid2">
      <div class="glass-card">
        <h2>24 小时分布</h2>
        <HourlyChart :slices="report?.hourly ?? []" />
      </div>
      <div class="glass-card">
        <h2>应用排行</h2>
        <AppRanking :apps="report?.apps ?? []" />
      </div>
    </section>
  </div>
</template>

<style scoped>
.today {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.head {
  display: flex;
  align-items: baseline;
  gap: 14px;
}

.head h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
}

.date {
  font-size: 12px;
  color: var(--text-muted);
}

/* 当前专注大分区 */
.focus {
  display: flex;
  align-items: center;
  gap: 28px;
  padding: 20px 24px;
}

.f-left {
  flex: 1.4;
  min-width: 0;
}

.f-status {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  padding: 3px 12px;
  border-radius: var(--r-full);
  margin-bottom: 10px;
}

.f-status .sdot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: currentColor;
}

.f-status.good {
  color: var(--good);
  background: var(--good-soft);
}

.f-status.warn {
  color: var(--warn);
  background: var(--warn-soft);
}

.f-status.muted {
  color: var(--text-faint);
  background: var(--surface);
}

.f-app {
  margin: 0 0 4px;
  font-size: 26px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.f-sub {
  margin: 0;
  font-size: 12px;
  color: var(--text-muted);
}

.f-mid {
  text-align: center;
  padding: 0 24px;
  border-left: 1px solid var(--border);
}

.f-num {
  margin: 0;
  font-size: 40px;
  font-weight: 700;
  color: var(--accent-text);
  font-variant-numeric: tabular-nums;
  line-height: 1.1;
}

.f-unit {
  margin: 0;
  font-size: 11px;
  color: var(--text-muted);
}

.f-right {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.f-kv {
  margin: 0;
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: var(--text-muted);
}

.f-kv b {
  color: var(--text);
  font-variant-numeric: tabular-nums;
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
  letter-spacing: 0.04em;
}

.stat .label {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 6px;
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

.stat .value.small {
  font-size: 16px;
}

.stat .value.num2 {
  font-size: 14px;
}

.stat small {
  font-size: 12px;
  font-weight: 400;
  color: var(--text-faint);
}

.grid2 {
  display: grid;
  grid-template-columns: 3fr 2fr;
  gap: 14px;
}
</style>
