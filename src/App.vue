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
import { appsTopN, isLight } from "./lib/uiState";

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
/** 外观参数版本：改默认值后要让旧值一次性失效，否则老用户永远看不到新观感 */
const UI_VERSION = "2";
const staleUi = localStorage.getItem("ui.v") !== UI_VERSION;
const themePref = ref<ThemePref>((localStorage.getItem("ui.theme") as ThemePref) || "dark");
const accent = ref(localStorage.getItem("ui.accent") || "indigo");
const glass = ref(localStorage.getItem("ui.glass") !== "off");
/** 玻璃模糊强度（px）与界面底色不透明度（%）
 *  模糊同时作用于卡片玻璃与背景柔光层——否则整屏都是纯色，模糊看不出变化 */
const blur = ref(Number(localStorage.getItem("ui.blur") ?? 26));
const bgAlpha = ref(staleUi ? 80 : Number(localStorage.getItem("ui.bgAlpha") ?? 80));
const collapsed = ref(localStorage.getItem("ui.side") === "collapsed");
const systemDark = ref(true);
let media: MediaQueryList | undefined;

const dark = () =>
  themePref.value === "dark" || (themePref.value === "system" && systemDark.value);
const naiveTheme = ref<GlobalTheme | null>(darkTheme);

watchEffect(() => {
  const root = document.documentElement;
  const isDarkNow = dark();
  root.dataset.theme = isDarkNow ? "dark" : "light";
  root.dataset.accent = accent.value;
  root.style.setProperty("--glass-blur", `${blur.value}px`);
  root.style.setProperty("--wall-blur", `${Math.round(blur.value * 0.7)}px`);
  root.style.setProperty("--bg-alpha", String(bgAlpha.value / 100));
  document.body.classList.toggle("glass-off", !glass.value);
  naiveTheme.value = isDarkNow ? darkTheme : null;
  isLight.value = !isDarkNow;
  localStorage.setItem("ui.theme", themePref.value);
  localStorage.setItem("ui.accent", accent.value);
  localStorage.setItem("ui.glass", glass.value ? "on" : "off");
  localStorage.setItem("ui.blur", String(blur.value));
  localStorage.setItem("ui.bgAlpha", String(bgAlpha.value));
  localStorage.setItem("ui.v", UI_VERSION);
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

/* ---------- 自绘标题栏（系统标题栏不跟随应用主题，会与界面"分割"开） ---------- */
const win = (() => {
  try {
    return getCurrentWebviewWindow();
  } catch {
    return null;
  }
})();
const maximized = ref(false);

/** 标题栏拖拽：走 Tauri 的 startDragging（与桌宠窗同一套，已实测可用） */
async function startDrag(e: MouseEvent) {
  if (e.buttons !== 1) return;
  try {
    await win?.startDragging();
  } catch {
    /* 忽略 */
  }
}

async function minimizeWin() {
  try {
    await win?.minimize();
  } catch {
    /* 忽略 */
  }
}
async function toggleMaxWin() {
  try {
    await win?.toggleMaximize();
    maximized.value = (await win?.isMaximized()) ?? false;
  } catch {
    /* 忽略 */
  }
}
async function closeWin() {
  try {
    await win?.close();
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
  try {
    unlistenState = await listen<{ paused: boolean }>("tracking-state", (e) => {
      paused.value = e.payload.paused;
      online.value = true;
    });
  } catch {
    /* 浏览器直开（无 Tauri 后端）时忽略 */
  }
  try {
    const p = await invoke<{ appsTopN: number }>("prefs_get");
    appsTopN.value = p.appsTopN;
  } catch {
    /* 用默认值 */
  }
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
      <div class="wall" aria-hidden="true"></div>

      <header class="titlebar">
        <div class="tb-drag" @mousedown="startDrag" @dblclick="toggleMaxWin">
          <span class="tb-logo">拾刻</span>
          <span class="tb-title">TallyMoment</span>
          <span class="tb-status" :class="status.tone">
            <span class="dot"></span>{{ status.text }}
          </span>
        </div>
        <div class="tb-btns">
          <button class="tb-btn" title="最小化" @click="minimizeWin">
            <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 6h8" stroke="currentColor" stroke-width="1.2" /></svg>
          </button>
          <button class="tb-btn" :title="maximized ? '还原' : '最大化'" @click="toggleMaxWin">
            <svg width="12" height="12" viewBox="0 0 12 12">
              <rect x="2.5" y="2.5" width="7" height="7" rx="1" fill="none" stroke="currentColor" stroke-width="1.2" />
            </svg>
          </button>
          <button class="tb-btn close" title="关闭（收进托盘，记录继续）" @click="closeWin">
            <svg width="12" height="12" viewBox="0 0 12 12">
              <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" />
            </svg>
          </button>
        </div>
      </header>

      <div class="body">
        <aside class="side" :class="{ collapsed }">
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
  /* 窗口本体透明，桌面由窗口特效透上来；界面底色交给 .wall 背景层 */
  background: transparent;
}

body.glass-off {
  background: rgb(var(--bg-rgb));
}

[data-theme="light"] {
  color-scheme: light;
}
</style>

<style scoped>
.shell {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
  border-radius: 10px;
  border: 1px solid var(--card-border);
}

.body {
  position: relative;
  z-index: 1;
  display: flex;
  flex: 1;
  min-height: 0;
}

/* ---------- 自绘标题栏 ---------- */
.titlebar {
  position: relative;
  z-index: 2;
  display: flex;
  align-items: center;
  height: 36px;
  flex: none;
  padding-left: 12px;
  background: var(--bg-glass);
  background: color-mix(in srgb, var(--bg-glass) 88%, transparent);
  border-bottom: 1px solid var(--card-border);
}

.tb-drag {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  height: 100%;
  cursor: default;
}

.tb-logo {
  font-size: 14px;
  font-weight: 700;
  letter-spacing: 0.1em;
}

.tb-title {
  font-size: 9px;
  letter-spacing: 0.22em;
  color: var(--text-faint);
  text-transform: uppercase;
}

.tb-status {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin-left: 10px;
  font-size: 10.5px;
  color: var(--text-faint);
}

.tb-status .dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-faint);
}

.tb-status.live {
  color: var(--good);
}
.tb-status.live .dot {
  background: var(--good);
  animation: pulse 2s infinite;
}
.tb-status.paused {
  color: var(--warn);
}
.tb-status.paused .dot {
  background: var(--warn);
}

.tb-btns {
  display: flex;
  height: 100%;
}

.tb-btn {
  width: 44px;
  height: 100%;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s, color 0.15s;
}

.tb-btn:hover {
  background: var(--surface-hover);
  color: var(--text);
}

.tb-btn.close:hover {
  background: var(--danger);
  color: #fff;
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

/* ---------- 侧栏 ---------- */
.side {
  flex: none;
  width: 200px;
  display: flex;
  flex-direction: column;
  padding: 14px 10px 12px;
  border-right: 1px solid var(--card-border);
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

.nav {
  display: flex;
  flex-direction: column;
  gap: 3px;
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
  transition: background 0.15s, color 0.15s, transform 0.15s;
  white-space: nowrap;
}

/* 微动效（参考 uiverse quick-fish-43 的 hover 放大 + 品牌色填充思路） */
.nav-item:hover {
  color: var(--text);
  background: var(--surface-hover);
  transform: translateX(2px);
}

.nav-item.active {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.nav-item.active:hover {
  transform: none;
}

.side.collapsed .nav-item {
  justify-content: center;
  padding: 9px 0;
}

.nav-label {
  overflow: hidden;
}

.side-foot {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

/* ---------- 内容区 ---------- */
.content {
  flex: 1;
  min-width: 0;
  padding: 16px 20px 20px;
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
