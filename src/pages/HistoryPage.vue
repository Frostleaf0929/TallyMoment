<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { DailyTotal, DayReport } from "../types";
import { fmtDuration } from "../lib/format";
import HeatTimeline from "../components/HeatTimeline.vue";
import HourlyChart from "../components/HourlyChart.vue";
import AppRanking from "../components/AppRanking.vue";

const days = ref<DailyTotal[]>([]);
const selected = ref("");
const report = ref<DayReport | null>(null);
const loading = ref(false);

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

onMounted(loadDays);
</script>

<template>
  <div class="history">
    <header class="head">
      <h1>历史</h1>
      <span class="sub">最近 14 天，选择一天回看</span>
    </header>

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
        <div class="card stat">
          <p class="label">当日总时长</p>
          <p class="value accent">{{ fmtDuration(report.totalSeconds) }}</p>
        </div>
        <div class="card stat">
          <p class="label">使用应用</p>
          <p class="value">{{ report.appCount }} <small>个</small></p>
        </div>
        <div class="card stat">
          <p class="label">键入 / 点击</p>
          <p class="value small num2">
            {{ report.keys }} <small>键</small> · {{ report.clicks }} <small>击</small>
          </p>
        </div>
      </section>

      <section class="card wide">
        <h2>时间线</h2>
        <HeatTimeline :segments="report.segments" />
      </section>

      <section class="grid2">
        <div class="card">
          <h2>24 小时分布</h2>
          <HourlyChart :slices="report.hourly" />
        </div>
        <div class="card">
          <h2>应用排行</h2>
          <AppRanking :apps="report.apps" />
        </div>
      </section>
    </template>
    <p v-else-if="loading" class="empty">加载中…</p>
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
  color: #6b7280;
}

.days {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.day {
  border: 1px solid var(--border);
  background: #161a24;
  color: #9aa3b8;
  border-radius: 10px;
  padding: 8px 12px;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 76px;
  font-family: inherit;
}

.day:hover {
  border-color: #39415a;
}

.day.active {
  border-color: rgba(99, 102, 241, 0.55);
  background: rgba(99, 102, 241, 0.12);
  color: #c7d2fe;
}

.dl {
  font-size: 12px;
  font-weight: 600;
}

.dv {
  font-size: 10px;
  color: #6b7280;
}

.cards {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 14px;
}

.card {
  border: 1px solid var(--border);
  background: rgba(22, 26, 36, 0.72);
  border-radius: 14px;
  padding: 16px 18px;
}

.card.wide {
  padding-bottom: 12px;
}

.card h2 {
  margin: 0 0 12px;
  font-size: 13px;
  font-weight: 600;
  color: #9aa3b8;
}

.stat .label {
  margin: 0 0 8px;
  font-size: 12px;
  color: #6b7280;
}

.stat .value {
  margin: 0;
  font-size: 22px;
  font-weight: 700;
  color: #e8eaf2;
  font-variant-numeric: tabular-nums;
}

.stat .value.accent {
  color: #34d399;
}

.stat .value.num2 {
  font-size: 14px;
}

.stat small {
  font-size: 12px;
  font-weight: 400;
  color: #6b7280;
}

.grid2 {
  display: grid;
  grid-template-columns: 3fr 2fr;
  gap: 14px;
}

.empty {
  color: var(--text-faint);
  font-size: 13px;
}
</style>
