<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { DayReport } from "../types";
import { fmtDuration } from "../lib/format";
import DayTimeline from "../components/DayTimeline.vue";
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

const dateLabel = () =>
  new Date().toLocaleDateString("zh-CN", {
    month: "long",
    day: "numeric",
    weekday: "long",
  });
const statusText = () =>
  !online.value
    ? "未连接"
    : report.value?.paused
      ? "已暂停"
      : report.value?.recording
        ? "记录中"
        : "待机";
</script>

<template>
  <div class="today">
    <header class="head">
      <h1>今日</h1>
      <span class="date">{{ dateLabel() }}</span>
      <span class="chip" :class="{ live: report?.recording && !report?.paused }">
        <span class="dot"></span>{{ statusText() }}
      </span>
    </header>

    <section class="cards">
      <div class="card stat">
        <p class="label">今日总时长</p>
        <p class="value accent">{{ fmtDuration(report?.totalSeconds ?? 0) }}</p>
      </div>
      <div class="card stat">
        <p class="label">使用应用</p>
        <p class="value">{{ report?.appCount ?? 0 }} <small>个</small></p>
      </div>
      <div class="card stat">
        <p class="label">当前使用</p>
        <p class="value small">
          {{ report?.current?.displayName?.replace(/\.exe$/i, "") ?? "—" }}
        </p>
      </div>
      <div class="card stat">
        <p class="label">键入 / 点击</p>
        <p class="value small num2">
          {{ report?.keys ?? 0 }} <small>键</small> · {{ report?.clicks ?? 0 }} <small>击</small>
        </p>
      </div>
    </section>

    <section class="card wide">
      <h2>今日时间线</h2>
      <DayTimeline :segments="report?.segments ?? []" />
    </section>

    <section class="grid2">
      <div class="card">
        <h2>24 小时分布</h2>
        <HourlyChart :slices="report?.hourly ?? []" />
      </div>
      <div class="card">
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
  align-items: center;
  gap: 14px;
}

.head h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
}

.date {
  font-size: 12px;
  color: #8b93a7;
}

.chip {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #9aa3b8;
  border: 1px solid #2a2f3d;
  background: #161a24;
  border-radius: 999px;
  padding: 4px 12px;
}

.chip .dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #6b7280;
}

.chip.live .dot {
  background: #34d399;
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0% {
    box-shadow: 0 0 0 0 rgba(52, 211, 153, 0.5);
  }
  70% {
    box-shadow: 0 0 0 6px rgba(52, 211, 153, 0);
  }
  100% {
    box-shadow: 0 0 0 0 rgba(52, 211, 153, 0);
  }
}

.cards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
}

.card {
  border: 1px solid #232936;
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
  letter-spacing: 0.04em;
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

.stat .value.small {
  font-size: 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
</style>
