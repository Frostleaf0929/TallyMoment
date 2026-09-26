<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { darkTheme, NConfigProvider } from "naive-ui";
import ChecklistCard from "./components/ChecklistCard.vue";
import DataCard from "./components/DataCard.vue";
import PetView from "./components/PetView.vue";
import ReminderCard from "./components/ReminderCard.vue";
import TodayPage from "./pages/TodayPage.vue";
import HistoryPage from "./pages/HistoryPage.vue";
import InsightsPage from "./pages/InsightsPage.vue";

// 提醒小窗/桌宠与主面板共用同一个前端入口，按窗口标签分流
let mode = "main";
try {
  const label = getCurrentWebviewWindow().label;
  if (label === "reminder") mode = "reminder";
  else if (label === "pet") mode = "pet";
} catch {
  /* 浏览器直开时按主面板处理 */
}

type Tab = "today" | "history" | "insights" | "checklist" | "data";
const tabs: { key: Tab; label: string }[] = [
  { key: "today", label: "今日" },
  { key: "history", label: "历史" },
  { key: "insights", label: "洞察" },
  { key: "checklist", label: "清单" },
  { key: "data", label: "数据" },
];
const active = ref<Tab>("today");

const online = ref(false);
let timer: number | undefined;

async function ping() {
  try {
    await invoke("today_report");
    online.value = true;
  } catch {
    online.value = false;
  }
}

onMounted(() => {
  if (mode !== "main") return;
  ping();
  timer = window.setInterval(ping, 10000);
});
onUnmounted(() => clearInterval(timer));
</script>

<template>
  <NConfigProvider :theme="darkTheme">
    <ReminderCard v-if="mode === 'reminder'" />
    <PetView v-else-if="mode === 'pet'" />
    <div v-else class="shell">
      <aside class="side">
        <div class="brand">
          <span class="logo">拾刻</span>
          <span class="en">TallyMoment</span>
        </div>
        <nav class="nav">
          <button
            v-for="t in tabs"
            :key="t.key"
            class="nav-item"
            :class="{ active: active === t.key }"
            @click="active = t.key"
          >
            {{ t.label }}
          </button>
        </nav>
        <div class="foot">
          <span class="dot" :class="{ live: online }"></span>
          <span>{{ online ? "记录运行中" : "未连接" }}</span>
        </div>
      </aside>
      <main class="content">
        <TodayPage v-show="active === 'today'" />
        <HistoryPage v-show="active === 'history'" />
        <InsightsPage v-show="active === 'insights'" />
        <div v-show="active === 'checklist'" class="page">
          <header class="phead">
            <h1>清单</h1>
            <span class="sub">到点在右下角弹提醒</span>
          </header>
          <div class="card">
            <ChecklistCard />
          </div>
        </div>
        <div v-show="active === 'data'" class="page">
          <header class="phead">
            <h1>数据</h1>
            <span class="sub">导入 / 导出 / 恢复</span>
          </header>
          <div class="card">
            <DataCard />
          </div>
        </div>
      </main>
    </div>
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
.shell {
  display: flex;
  min-height: 100vh;
  background: #0f1117;
}

.side {
  flex: none;
  width: 148px;
  display: flex;
  flex-direction: column;
  padding: 18px 12px;
  border-right: 1px solid #1c212d;
  background:
    radial-gradient(240px 200px at 0% 0%, rgba(99, 102, 241, 0.1), transparent 65%),
    #0d0f15;
}

.brand {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 2px 8px 18px;
}

.logo {
  font-size: 22px;
  font-weight: 700;
  letter-spacing: 0.14em;
}

.en {
  font-size: 9px;
  letter-spacing: 0.28em;
  color: #4b5563;
  text-transform: uppercase;
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-item {
  text-align: left;
  border: 0;
  background: transparent;
  color: #9aa3b8;
  font-size: 14px;
  font-family: inherit;
  padding: 9px 12px;
  border-radius: 9px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.nav-item:hover {
  color: #e8eaf2;
  background: #161a24;
}

.nav-item.active {
  color: #c7d2fe;
  background: rgba(99, 102, 241, 0.16);
}

.foot {
  margin-top: auto;
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 11px;
  color: #6b7280;
  padding: 0 8px;
}

.foot .dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #6b7280;
}

.foot .dot.live {
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

.content {
  flex: 1;
  min-width: 0;
  padding: 18px 22px 22px;
  overflow-y: auto;
  background:
    radial-gradient(700px 320px at 85% -5%, rgba(99, 102, 241, 0.08), transparent 60%),
    #0f1117;
}

.page {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.phead {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.phead h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
}

.sub {
  font-size: 12px;
  color: #6b7280;
}

.card {
  border: 1px solid #232936;
  background: rgba(22, 26, 36, 0.72);
  border-radius: 14px;
  padding: 16px 18px;
}
</style>
