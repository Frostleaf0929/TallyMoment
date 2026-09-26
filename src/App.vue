<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { darkTheme, NConfigProvider } from "naive-ui";
import type { DayReport } from "./types";
import { fmtDuration } from "./lib/format";
import DayTimeline from "./components/DayTimeline.vue";
import HourlyChart from "./components/HourlyChart.vue";
import AppRanking from "./components/AppRanking.vue";
import ChecklistCard from "./components/ChecklistCard.vue";
import DataCard from "./components/DataCard.vue";
import PetView from "./components/PetView.vue";
import ReminderCard from "./components/ReminderCard.vue";

// 提醒小窗/桌宠与主面板共用同一个前端入口，按窗口标签分流
let mode = "main";
try {
  const label = getCurrentWebviewWindow().label;
  if (label === "reminder") mode = "reminder";
  else if (label === "pet") mode = "pet";
} catch {
  /* 浏览器直开时按主面板处理 */
}

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
  if (mode !== "main") return;
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
  <NConfigProvider :theme="darkTheme">
    <ReminderCard v-if="mode === 'reminder'" />
    <PetView v-else-if="mode === 'pet'" />
    <main v-else class="dash">
      <header class="top">
        <div class="brand">
          <span class="logo">拾刻</span>
          <span class="en">TallyMoment</span>
        </div>
        <div class="meta">
          <span class="date">{{ dateLabel() }}</span>
          <span class="chip" :class="{ live: report?.recording && !report?.paused }">
            <span class="dot"></span>{{ statusText() }}
          </span>
        </div>
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
          <p class="label">记录状态</p>
          <p class="value small">{{ statusText() }}</p>
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

      <section class="card wide">
        <h2>我的清单（到点在右下角弹提醒）</h2>
        <ChecklistCard />
      </section>

      <section class="card wide">
        <h2>数据管理</h2>
        <DataCard />
      </section>
    </main>
  </NConfigProvider>
</template>

<style>
:root {
  font-family: "Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei", system-ui, sans-serif;
  font-size: 16px;
  color: #e8eaf2;
  background-color: #0f1117;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  user-select: none;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
}
</style>

<style scoped>
.dash {
  min-height: 100vh;
  padding: 18px 22px 22px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  background:
    radial-gradient(700px 320px at 15% -5%, rgba(99, 102, 241, 0.12), transparent 60%),
    radial-gradient(700px 320px at 90% 105%, rgba(16, 185, 129, 0.08), transparent 60%),
    #0f1117;
}

.top {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.brand {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.logo {
  font-size: 20px;
  font-weight: 700;
  letter-spacing: 0.14em;
}

.en {
  font-size: 11px;
  letter-spacing: 0.32em;
  color: #6b7280;
  text-transform: uppercase;
}

.meta {
  display: flex;
  align-items: center;
  gap: 14px;
}

.date {
  font-size: 12px;
  color: #8b93a7;
}

.chip {
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
