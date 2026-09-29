<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch, watchEffect } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { darkTheme, dateZhCN, NConfigProvider, zhCN, type GlobalTheme } from "naive-ui";
import Icon from "./components/Icon.vue";
import PetView from "./components/PetView.vue";
import ToastStack from "./components/ToastStack.vue";
import FullscreenReminder from "./components/FullscreenReminder.vue";
import TodayPage from "./pages/TodayPage.vue";
import HistoryPage from "./pages/HistoryPage.vue";
import DetailPage from "./pages/DetailPage.vue";
import InsightsPage from "./pages/InsightsPage.vue";
import TodoPage from "./pages/TodoPage.vue";
import SettingsPage from "./pages/SettingsPage.vue";
import PersonalizePage from "./pages/PersonalizePage.vue";
import { activeTab, appsTopN, isLight, navIntent, tabIntent } from "./lib/uiState";
import {
  applyAppearance,
  glass,
  isDark,
  saveAppearance,
  systemDark,
  wallImageUrl,
  wallKind,
} from "./lib/appearance";

// 提醒小窗/桌宠与主面板共用同一个前端入口，按窗口标签分流
let mode = "main";
try {
  const label = getCurrentWebviewWindow().label;
  if (label === "reminder") mode = "reminder";
  else if (label === "reminder_full") mode = "reminder_full";
  else if (label === "pet") mode = "pet";
} catch {
  /* 浏览器直开时按主面板处理 */
}
document.documentElement.dataset.mode = mode;

type Tab =
  | "today"
  | "history"
  | "detail"
  | "insights"
  | "todo"
  | "personalize"
  | "settings";
const tabs: { key: Tab; label: string; icon: string }[] = [
  { key: "today", label: "今日", icon: "clock" },
  { key: "todo", label: "待办", icon: "checklist" },
  { key: "history", label: "历史", icon: "history" },
  { key: "detail", label: "详细", icon: "detail" },
  { key: "insights", label: "洞察", icon: "graph" },
  { key: "personalize", label: "个性化", icon: "palette" },
];
const active = ref<Tab>("today");

// 今日卡片 / 应用排行点击 → 跳到历史页对应视图
// "返回待办"这类按钮：直接切页
watchEffect(() => {
  const t = tabIntent.value;
  if (t) active.value = t.tab as Tab;
});

watchEffect(() => {
  const n = navIntent.value;
  if (!n) return;
  // 应用排行 / 某天弹层放大 / 任务名 → 详细页；其余 → 历史页
  active.value = n.view === "app" || n.view === "items" || n.view === "task" ? "detail" : "history";
});
const collapsed = ref(localStorage.getItem("ui.side") === "collapsed");

// 当前页广播给各页面（v-show 页面感知"自己被切到"时刷新数据）
watch(active, (t) => (activeTab.value = t));

const naiveTheme = ref<GlobalTheme | null>(darkTheme);
watchEffect(() => {
  applyAppearance();
  saveAppearance();
  naiveTheme.value = isDark() ? darkTheme : null;
  isLight.value = !isDark();
});

function toggleCollapse() {
  collapsed.value = !collapsed.value;
  localStorage.setItem("ui.side", collapsed.value ? "collapsed" : "expanded");
}

/* ---------- 窗口控制（权限已在 capabilities 显式声明） ---------- */
const win = (() => {
  try {
    return getCurrentWebviewWindow();
  } catch {
    return null;
  }
})();
const maximized = ref(false);
const winErr = ref("");

/* 拖窗：必须"按住 + 移动 ≥5px"才触发，单击（含按钮上、文字旁）绝不会移动窗口 */
let pressFrom: { x: number; y: number } | null = null;

function onBrandMove(e: MouseEvent) {
  if (!pressFrom) return;
  if (Math.abs(e.clientX - pressFrom.x) < 5 && Math.abs(e.clientY - pressFrom.y) < 5) return;
  pressFrom = null;
  window.removeEventListener("mousemove", onBrandMove);
  void win?.startDragging();
}

function onBrandUp() {
  pressFrom = null;
  window.removeEventListener("mousemove", onBrandMove);
}

function onBrandDown(e: MouseEvent) {
  if (e.buttons !== 1 || e.detail > 1) return;
  const t = e.target as HTMLElement | null;
  if (t?.closest("button, input, a, .collapse-btn, .logo-btn")) return;
  pressFrom = { x: e.clientX, y: e.clientY };
  window.addEventListener("mousemove", onBrandMove);
  window.addEventListener("mouseup", onBrandUp, { once: true });
}

async function minimizeWin() {
  winErr.value = "";
  try {
    await win?.minimize();
  } catch (e) {
    winErr.value = String(e);
  }
}

async function toggleMaxWin() {
  winErr.value = "";
  try {
    await win?.toggleMaximize();
    maximized.value = (await win?.isMaximized()) ?? false;
  } catch (e) {
    winErr.value = String(e);
  }
}

async function closeWin() {
  winErr.value = "";
  try {
    await win?.close();
  } catch (e) {
    winErr.value = String(e);
  }
}

/* ---------- 运行状态 ---------- */
const online = ref(false);
const paused = ref(false);
let timer: number | undefined;
let unlistenState: UnlistenFn | undefined;
let unlistenIsland: UnlistenFn | undefined;

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

async function loadWallpaper() {
  if (wallKind.value !== "image") return;
  try {
    const w = await invoke<{ mime: string; data: string } | null>("wallpaper_get");
    if (!w) return;
    const bin = atob(w.data);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    if (wallImageUrl.value) URL.revokeObjectURL(wallImageUrl.value);
    wallImageUrl.value = URL.createObjectURL(new Blob([bytes], { type: w.mime }));
  } catch {
    /* 忽略 */
  }
}

onMounted(async () => {
  if (mode !== "main") return;
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  systemDark.value = media.matches;
  media.addEventListener("change", (e) => (systemDark.value = e.matches));
  ping();
  timer = window.setInterval(ping, 5000);
  try {
    unlistenState = await listen<{ paused: boolean }>("tracking-state", (e) => {
      paused.value = e.payload.paused;
      online.value = true;
    });
  } catch {
    /* 浏览器直开时忽略 */
  }
  try {
    const p = await invoke<{ appsTopN: number }>("prefs_get");
    appsTopN.value = p.appsTopN;
  } catch {
    /* 用默认值 */
  }
  // 原子岛展开卡里的「打开设置」：跳到个性化
  try {
    unlistenIsland = await listen("island-open-settings", () => (active.value = "personalize"));
  } catch {
    /* 浏览器直开时忽略 */
  }
  await loadWallpaper();
});
onUnmounted(() => {
  clearInterval(timer);
  unlistenState?.();
  unlistenIsland?.();
  window.removeEventListener("mousemove", onBrandMove);
  window.removeEventListener("mouseup", onBrandUp);
});
</script>

<template>
  <NConfigProvider :theme="naiveTheme" :locale="zhCN" :date-locale="dateZhCN">
    <FullscreenReminder v-if="mode === 'reminder_full'" />
    <ToastStack v-else-if="mode === 'reminder'" />
    <PetView v-else-if="mode === 'pet'" />
    <div v-else class="shell shell-frame" :class="{ 'glass-off': !glass }">
      <div class="wall" aria-hidden="true"></div>

      <!-- 窗口控制：悬浮在右上角，不占一整条标题栏，避免多出一道"割裂面" -->
      <div class="winbtns">
        <button class="wb" title="最小化" @click="minimizeWin">
          <svg width="11" height="11" viewBox="0 0 12 12"><path d="M2 6h8" stroke="currentColor" stroke-width="1.2" /></svg>
        </button>
        <button class="wb" :title="maximized ? '还原' : '最大化'" @click="toggleMaxWin">
          <svg width="11" height="11" viewBox="0 0 12 12">
            <rect x="2.5" y="2.5" width="7" height="7" rx="1" fill="none" stroke="currentColor" stroke-width="1.2" />
          </svg>
        </button>
        <button class="wb close" title="关闭（收进托盘，记录继续）" @click="closeWin">
          <svg width="11" height="11" viewBox="0 0 12 12">
            <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" />
          </svg>
        </button>
      </div>

      <aside class="side" :class="{ collapsed }">
        <div class="brand" @mousedown="onBrandDown" @dblclick="toggleMaxWin">
          <!-- 折叠时：应用图标就是"展开"入口 -->
          <button
            v-if="collapsed"
            class="logo-btn"
            title="展开侧栏"
            @mousedown.stop
            @click="toggleCollapse"
          >
            <img class="logoimg" src="/app-icon.svg" alt="拾刻" draggable="false" />
          </button>
          <!-- 展开时：品牌文字 + 原来的收起图标（不放应用图标） -->
          <template v-else>
            <span class="logo">拾刻</span>
            <span class="en">TallyMoment</span>
            <button
              class="collapse-btn"
              title="收起侧栏"
              @mousedown.stop
              @click="toggleCollapse"
            >
              <Icon name="collapse" :size="16" />
            </button>
          </template>
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
          <button class="nav-item" :class="{ active: active === 'settings' }" title="设置" @click="active = 'settings'">
            <Icon name="settings" :size="19" />
            <span v-if="!collapsed" class="nav-label">设置</span>
          </button>
          <div class="status" :class="status.tone" :title="status.text">
            <span class="dot"></span>
            <span v-if="!collapsed">{{ status.text }}</span>
          </div>
        </div>
      </aside>

      <main class="content">
        <TodayPage v-show="active === 'today'" @jump="() => (active = 'history')" />
        <HistoryPage v-show="active === 'history'" />
        <DetailPage v-show="active === 'detail'" />
        <InsightsPage v-show="active === 'insights'" />
        <TodoPage v-show="active === 'todo'" />
        <!-- 桌宠模块已停用（用户决定降优先级，代码保留在 pages/PetSettingsPage.vue） -->
        <PersonalizePage v-show="active === 'personalize'" @wallpaper-changed="loadWallpaper" />
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
/* 整窗一块材质：侧栏与内容区都直接贴在 .wall 背景层上，不再各自刷一层底 */
.shell {
  position: relative;
  display: flex;
  height: 100vh;
  /* 16px 留白做成"外框"，侧栏与内容区各自是一块圆角卡片（对标参考图的卡片式布局） */
  padding: 16px;
  gap: 16px;
  overflow: hidden;
  /* 外围那层不再自己做圆角与描边：整窗只有一层背景（.wall）+ 两块圆角卡片 */
  border-radius: 0;
  border: 0;
  box-sizing: border-box;
}

.side {
  position: relative;
  z-index: 1;
  flex: none;
  width: 200px;
  display: flex;
  flex-direction: column;
  padding: 10px 10px 12px;
  border: 1px solid var(--card-border);
  border-radius: 16px;
  box-shadow: var(--card-shadow);
  /* 与卡片同一套材质：背景层透上来 + 同一档模糊 */
  background: rgba(var(--side-rgb), var(--side-alpha));
  backdrop-filter: blur(var(--glass-blur)) saturate(1.25);
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(1.25);
  transition: width var(--dur) cubic-bezier(0.22, 0.61, 0.36, 1);
  overflow: hidden;
}

[data-material="liquid"] .side {
  backdrop-filter: blur(calc(var(--glass-blur) * 0.55)) saturate(1.75) brightness(1.06);
  -webkit-backdrop-filter: blur(calc(var(--glass-blur) * 0.55)) saturate(1.75) brightness(1.06);
}

.glass-off .side {
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.side.collapsed {
  width: 64px;
}

/* 侧栏左侧的竖线随激活项——留在卡片内部 */
.side {
  overflow: hidden;
}

/* 品牌行兼作拖拽区（没有独立标题栏了） */
.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 40px;
  padding: 4px 4px 8px 10px;
  white-space: nowrap;
  cursor: default;
}

/* 应用图标本身就是折叠/展开开关 */
.logo-btn {
  border: 0;
  background: transparent;
  padding: 0;
  border-radius: 8px;
  cursor: pointer;
  display: inline-flex;
  flex: none;
  transition: transform var(--dur), box-shadow var(--dur);
}

.logo-btn:hover {
  transform: scale(calc(1 + 0.06 * var(--motion)));
}

.logoimg {
  width: 26px;
  height: 26px;
  border-radius: 8px;
  display: block;
}

/* 展开状态下的收起按钮（沿用原来的双箭头图标） */
.collapse-btn {
  margin-left: auto;
  border: 0;
  background: transparent;
  color: var(--text-faint);
  border-radius: var(--r-sm);
  width: 24px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex: none;
  transition: background var(--dur), color var(--dur);
}

.collapse-btn:hover {
  background: var(--surface-hover);
  color: var(--text);
}

.side.collapsed .brand {
  justify-content: center;
  padding: 4px 0 8px;
}

.logo {
  font-size: 19px;
  font-weight: 700;
  letter-spacing: 0.14em;
}

.en {
  font-size: 9px;
  letter-spacing: 0.24em;
  color: var(--text-faint);
  text-transform: uppercase;
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 3px;
  margin-top: 4px;
}

.nav-item {
  position: relative;
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
  transition: background var(--dur), color var(--dur), transform var(--dur);
  white-space: nowrap;
}

/* 视觉引导：整块侧栏左侧的竖线随激活项移动（对标 Tai 的竖向指示条） */
.side::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 1px;
  background: linear-gradient(
    to bottom,
    transparent,
    var(--border-strong) 12%,
    var(--border-strong) 88%,
    transparent
  );
}

.nav-item.active::before {
  content: "";
  position: absolute;
  left: -9px;
  top: 50%;
  transform: translateY(-50%);
  width: 3px;
  height: 20px;
  border-radius: 0 3px 3px 0;
  background: var(--accent);
  box-shadow: 0 0 10px var(--accent-soft);
}

.nav-item:hover {
  color: var(--text);
  background: var(--surface-hover);
  transform: translateX(calc(2px * var(--motion)));
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
  position: relative;
  z-index: 1;
  flex: 1;
  min-width: 0;
  /* 内容区也是一块圆角卡片 */
  border: 1px solid var(--card-border);
  border-radius: 16px;
  background: color-mix(in srgb, var(--bg-glass) 55%, transparent);
  box-shadow: var(--card-shadow);
  /* 顶部留出悬浮按钮的高度 */
  padding: 38px 20px 20px;
  overflow-y: auto;
}

/* ---------- 悬浮窗口控制按钮 ---------- */
.winbtns {
  position: absolute;
  top: 22px;
  right: 22px;
  z-index: 5;
  display: flex;
  gap: 2px;
  border-radius: var(--r-sm);
  transition: background var(--dur);
}

.wb {
  width: 30px;
  height: 24px;
  border: 0;
  background: transparent;
  color: var(--text-faint);
  border-radius: 6px;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: background var(--dur), color var(--dur);
}

.winbtns:hover .wb {
  color: var(--text-muted);
}

.wb:hover {
  background: var(--surface-hover);
  color: var(--text);
}

.wb.close:hover {
  background: var(--danger);
  color: #fff;
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
