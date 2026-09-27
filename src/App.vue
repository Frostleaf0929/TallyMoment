<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watchEffect } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { darkTheme, dateZhCN, NConfigProvider, zhCN, type GlobalTheme } from "naive-ui";
import Icon from "./components/Icon.vue";
import DataCard from "./components/DataCard.vue";
import PetView from "./components/PetView.vue";
import ToastStack from "./components/ToastStack.vue";
import TodayPage from "./pages/TodayPage.vue";
import HistoryPage from "./pages/HistoryPage.vue";
import InsightsPage from "./pages/InsightsPage.vue";
import TodoPage from "./pages/TodoPage.vue";
import SettingsPage from "./pages/SettingsPage.vue";
import PersonalizePage from "./pages/PersonalizePage.vue";
import PetSettingsPage from "./pages/PetSettingsPage.vue";

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

type Tab =
  | "today"
  | "history"
  | "insights"
  | "todo"
  | "data"
  | "petsettings"
  | "personalize"
  | "settings";
const tabs: { key: Tab; label: string; icon: string }[] = [
  { key: "today", label: "今日", icon: "clock" },
  { key: "history", label: "历史", icon: "doc" },
  { key: "insights", label: "洞察", icon: "graph" },
  { key: "todo", label: "待办", icon: "checklist" },
  { key: "data", label: "数据", icon: "database" },
  { key: "petsettings", label: "桌宠", icon: "cat" },
  { key: "personalize", label: "个性化", icon: "palette" },
];
const active = ref<Tab>("today");

/* ---------- 外观（主题/强调色/毛玻璃），持久化到 localStorage ---------- */
type ThemePref = "dark" | "light" | "system";
const themePref = ref<ThemePref>((localStorage.getItem("ui.theme") as ThemePref) || "dark");
const accent = ref(localStorage.getItem("ui.accent") || "indigo");
const glass = ref(localStorage.getItem("ui.glass") !== "off");
/** 玻璃模糊强度（px）与界面底色不透明度（%）——毛玻璃可调参数
 *  底色默认 80%：留出一点透光，窗口特效才看得出来；调满 100% 就是传统实底观感 */
const blur = ref(Number(localStorage.getItem("ui.blur") ?? 26));
const bgAlpha = ref(Number(localStorage.getItem("ui.bgAlpha") ?? 80));
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
  root.style.setProperty("--glass-blur", `${blur.value}px`);
  root.style.setProperty("--bg-alpha", String(bgAlpha.value / 100));
  document.body.classList.toggle("glass-off", !glass.value);
  naiveTheme.value = isDark() ? darkTheme : null;
  localStorage.setItem("ui.theme", themePref.value);
  localStorage.setItem("ui.accent", accent.value);
  localStorage.setItem("ui.glass", glass.value ? "on" : "off");
  localStorage.setItem("ui.blur", String(blur.value));
  localStorage.setItem("ui.bgAlpha", String(bgAlpha.value));
});

function toggleCollapse() {
  collapsed.value = !collapsed.value;
  localStorage.setItem("ui.side", collapsed.value ? "collapsed" : "expanded");
}

const petVisible = ref(true);
async function togglePet() {
  try {
    petVisible.value = await invoke<boolean>("toggle_pet");
  } catch {
    /* 忽略 */
  }
}

/* ---------- 运行状态：记录中 / 已暂停 / 未连接 ---------- */
const online = ref(false);
const paused = ref(false);
let timer: number | undefined;
let unlistenState: UnlistenFn | undefined;

async function ping() {
  try {
    const s = await invoke<{ paused: boolean }>("tracking_state");
    paused.value = s.paused;
    online.value = true;
  } catch {
    online.value = false;
  }
}

const status = computed(() => {
  if (!online.value) return { text: "未连接", tone: "off" };
  if (paused.value) return { text: "已暂停", tone: "paused" };
  return { text: "记录中", tone: "live" };
});

onMounted(async () => {
  if (mode !== "main") return;
  media = window.matchMedia("(prefers-color-scheme: dark)");
  systemDark.value = media.matches;
  const onScheme = (e: MediaQueryListEvent) => (systemDark.value = e.matches);
  media.addEventListener("change", onScheme);
  ping();
  timer = window.setInterval(ping, 5000);
  // 托盘切暂停时即时更新，不必等下一次轮询
  unlistenState = await listen<{ paused: boolean }>("tracking-state", (e) => {
    paused.value = e.payload.paused;
    online.value = true;
  });
});
onUnmounted(() => {
  clearInterval(timer);
  unlistenState?.();
});
</script>

<template>
  <NConfigProvider :theme="naiveTheme" :locale="zhCN" :date-locale="dateZhCN">
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
          <button class="nav-item" title="设置" @click="active = 'settings'">
            <Icon name="settings" :size="19" />
            <span v-if="!collapsed" class="nav-label">设置</span>
          </button>
          <button class="nav-item" :title="collapsed ? '展开侧栏' : '收起侧栏'" @click="toggleCollapse">
            <Icon :name="collapsed ? 'expand' : 'collapse'" :size="19" />
            <span v-if="!collapsed" class="nav-label">收起</span>
          </button>
          <div class="status" :class="status.tone" :title="status.text">
            <span class="dot"></span>
            <span v-if="!collapsed">{{ status.text }}</span>
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
        <PetSettingsPage
          v-show="active === 'petsettings'"
          v-model:pet-visible="petVisible"
          @toggle-pet="togglePet"
        />
        <PersonalizePage
          v-show="active === 'personalize'"
          v-model:theme-pref="themePref"
          v-model:accent="accent"
          v-model:glass="glass"
          v-model:blur="blur"
          v-model:bg-alpha="bgAlpha"
        />
        <SettingsPage v-show="active === 'settings'" />
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
  /* 半透明底 + 窗口特效，才能让桌面透过窗口（此前是不透明的 #0f1116，
     把整块客户区糊死，只剩没被 webview 覆盖的标题栏能看见亚克力） */
  background: rgba(var(--bg-rgb), var(--bg-alpha, 1));
  transition: background 0.2s;
}

/* 毛玻璃关闭时完全实底，遮住窗口特效 */
body.glass-off {
  background: rgb(var(--bg-rgb));
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

.status.live {
  color: var(--good);
}

.status.live .dot {
  background: var(--good);
  animation: pulse 2s infinite;
}

.status.paused {
  color: var(--warn);
}

.status.paused .dot {
  background: var(--warn);
}

.status.off {
  color: var(--text-faint);
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
