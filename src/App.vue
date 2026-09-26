<script setup lang="ts">
import { onMounted, onUnmounted, ref, watchEffect } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { darkTheme, NConfigProvider, type GlobalTheme } from "naive-ui";
import Icon from "./components/Icon.vue";
import DataCard from "./components/DataCard.vue";
import PetView from "./components/PetView.vue";
import ToastStack from "./components/ToastStack.vue";
import TodayPage from "./pages/TodayPage.vue";
import HistoryPage from "./pages/HistoryPage.vue";
import InsightsPage from "./pages/InsightsPage.vue";
import TodoPage from "./pages/TodoPage.vue";
import SettingsPage from "./pages/SettingsPage.vue";

// 提醒小窗/桌宠与主面板共用同一个前端入口，按窗口标签分流
let mode = "main";
try {
  const label = getCurrentWebviewWindow().label;
  if (label === "reminder") mode = "reminder";
  else if (label === "pet") mode = "pet";
} catch {
  /* 浏览器直开时按主面板处理 */
}
// 标记到 <html>，供 theme.css 让提醒/桌宠窗体透明
document.documentElement.dataset.mode = mode;

type Tab = "today" | "history" | "insights" | "todo" | "data" | "settings";
const tabs: { key: Tab; label: string; icon: string }[] = [
  { key: "today", label: "今日", icon: "clock" },
  { key: "history", label: "历史", icon: "doc" },
  { key: "insights", label: "洞察", icon: "graph" },
  { key: "todo", label: "待办", icon: "checklist" },
  { key: "data", label: "数据", icon: "database" },
];
const active = ref<Tab>("today");

/* ---------- 个性化（主题/强调色/毛玻璃/侧栏），持久化到 localStorage ---------- */
type ThemePref = "dark" | "light" | "system";
const themePref = ref<ThemePref>((localStorage.getItem("ui.theme") as ThemePref) || "dark");
const accent = ref(localStorage.getItem("ui.accent") || "indigo");
const glass = ref(localStorage.getItem("ui.glass") !== "off");
const collapsed = ref(localStorage.getItem("ui.side") === "collapsed");
const systemDark = ref(true);
let media: MediaQueryList | undefined;

const isDark = () =>
  themePref.value === "dark" || (themePref.value === "system" && systemDark.value);
const naiveTheme = ref<GlobalTheme | null>(darkTheme);

watchEffect(() => {
  const root = document.documentElement;
  root.dataset.theme = isDark() ? "dark" : "light";
  root.dataset.accent = accent.value;
  document.body.classList.toggle("glass-off", !glass.value);
  naiveTheme.value = isDark() ? darkTheme : null;
  localStorage.setItem("ui.theme", themePref.value);
  localStorage.setItem("ui.accent", accent.value);
  localStorage.setItem("ui.glass", glass.value ? "on" : "off");
});

function toggleCollapse() {
  collapsed.value = !collapsed.value;
  localStorage.setItem("ui.side", collapsed.value ? "collapsed" : "expanded");
}

function toggleGlass() {
  glass.value = !glass.value;
}

const petVisible = ref(true);
async function togglePet() {
  try {
    petVisible.value = await invoke<boolean>("toggle_pet");
  } catch {
    /* 忽略 */
  }
}

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
  media = window.matchMedia("(prefers-color-scheme: dark)");
  systemDark.value = media.matches;
  const onScheme = (e: MediaQueryListEvent) => (systemDark.value = e.matches);
  media.addEventListener("change", onScheme);
  ping();
  timer = window.setInterval(ping, 10000);
});
onUnmounted(() => clearInterval(timer));
</script>

<template>
  <NConfigProvider :theme="naiveTheme">
    <ToastStack v-if="mode === 'reminder'" />
    <PetView v-else-if="mode === 'pet'" />
    <div v-else class="shell" :class="{ 'glass-off': !glass }">
      <aside class="side" :class="{ collapsed }">
        <div class="brand">
          <span class="logo">拾刻</span>
          <span v-if="!collapsed" class="en">TallyMoment</span>
        </div>
        <nav class="nav">
          <button
            v-for="t in tabs"
            :key="t.key"
            class="nav-item"
            :class="{ active: active === t.key }"
            :title="t.label"
            @click="active = t.key"
          >
            <Icon :name="t.icon" :size="19" />
            <span v-if="!collapsed" class="nav-label">{{ t.label }}</span>
          </button>
        </nav>
        <div class="side-foot">
          <button class="nav-item" title="个性化" @click="active = 'settings'">
            <Icon name="palette" :size="19" />
            <span v-if="!collapsed" class="nav-label">个性化</span>
          </button>
          <button class="nav-item" :title="collapsed ? '展开侧栏' : '收起侧栏'" @click="toggleCollapse">
            <Icon :name="collapsed ? 'expand' : 'collapse'" :size="19" />
            <span v-if="!collapsed" class="nav-label">收起</span>
          </button>
          <div class="status" :title="online ? '记录服务正常' : '未连接'">
            <span class="dot" :class="{ live: online }"></span>
            <span v-if="!collapsed">{{ online ? "记录中" : "未连接" }}</span>
          </div>
        </div>
      </aside>
      <main class="content">
        <TodayPage v-show="active === 'today'" />
        <HistoryPage v-show="active === 'history'" />
        <InsightsPage v-show="active === 'insights'" />
        <TodoPage v-show="active === 'todo'" />
        <div v-show="active === 'data'" class="page">
          <header class="phead">
            <h1>数据</h1>
            <span class="sub">导出 / 恢复 / 删除</span>
          </header>
          <div class="glass-card card">
            <DataCard />
          </div>
        </div>
        <SettingsPage v-show="active === 'settings'" v-model:theme-pref="themePref"
          v-model:accent="accent" v-model:glass="glass" v-model:pet-visible="petVisible"
          @toggle-glass="toggleGlass" @toggle-pet="togglePet" />
      </main>
    </div>
  </NConfigProvider>
</template>

<style>
:root {
  color-scheme: dark;
  font-family: "Segoe UI Variable", "Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei",
    system-ui, sans-serif;
  font-size: 16px;
  color: var(--text);
  background-color: transparent;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  user-select: none;
}

body {
  margin: 0;
  background: var(--bg);
  transition: background 0.2s;
}

/* 毛玻璃关闭时完全实底，遮住窗口特效 */
body.glass-off {
  background: var(--bg);
}

[data-theme="light"] {
  color-scheme: light;
}
</style>

<style scoped>
.shell {
  display: flex;
  height: 100vh;
}

.side {
  flex: none;
  width: 200px;
  display: flex;
  flex-direction: column;
  padding: 16px 10px 14px;
  border-right: 1px solid var(--border);
  background: var(--bg-glass);
  backdrop-filter: blur(var(--glass-blur)) saturate(1.25);
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(1.25);
  transition: width 0.18s ease;
  overflow: hidden;
}

.glass-off .side {
  background: var(--surface-solid);
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.side.collapsed {
  width: 64px;
}

.brand {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 4px 10px 16px;
  white-space: nowrap;
}

.logo {
  font-size: 21px;
  font-weight: 700;
  letter-spacing: 0.14em;
}

.en {
  font-size: 9px;
  letter-spacing: 0.26em;
  color: var(--text-faint);
  text-transform: uppercase;
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  text-align: left;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 14px;
  font-family: inherit;
  padding: 9px 12px;
  border-radius: var(--r-md);
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
  white-space: nowrap;
}

.side.collapsed .nav-item {
  justify-content: center;
  padding: 9px 0;
}

.nav-item:hover {
  color: var(--text);
  background: var(--surface-hover);
}

.nav-item.active {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.nav-label {
  overflow: hidden;
}

.side-foot {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.status {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--text-faint);
  padding: 8px 12px 2px;
  white-space: nowrap;
}

.side.collapsed .status {
  justify-content: center;
  padding: 8px 0 2px;
}

.status .dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--text-faint);
  flex: none;
}

.status .dot.live {
  background: var(--good);
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0% {
    box-shadow: 0 0 0 0 rgba(111, 181, 154, 0.5);
  }
  70% {
    box-shadow: 0 0 0 6px rgba(111, 181, 154, 0);
  }
  100% {
    box-shadow: 0 0 0 0 rgba(111, 181, 154, 0);
  }
}

.content {
  flex: 1;
  min-width: 0;
  padding: 18px 22px 22px;
  overflow-y: auto;
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
  color: var(--text-muted);
}

.card {
  padding: 16px 18px;
}
</style>
