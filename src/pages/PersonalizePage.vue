<script setup lang="ts">
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { currentMonitor } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import { NSwitch } from "naive-ui";
import Icon from "../components/Icon.vue";
import SettingSlider from "../components/SettingSlider.vue";
import { appsTopN } from "../lib/uiState";
import { onMounted } from "vue";
import {
  accent,
  accentIsPreset,
  applySkin,
  activeSkinKey,
  bgAlpha,
  cardAlpha,
  bgBlur,
  sideBlur,
  cardBlur,
  cardShadow,
  appColorMode,
  exportPack,
  glass,
  importPack,
  material,
  motion,
  sideAlpha,
  skins,
  themePref,
  wallKind,
  wallImageUrl,
  type ThemePack,
} from "../lib/appearance";

const emit = defineEmits<{ (e: "wallpaper-changed"): void }>();

const msg = ref("");
const reminderBgSet = ref(false);
const err = ref("");

const themes: { key: "dark" | "light" | "system"; label: string; icon: string }[] = [
  { key: "dark", label: "深色", icon: "moon" },
  { key: "light", label: "浅色", icon: "sun" },
  { key: "system", label: "跟随系统", icon: "settings" },
];

const accentPresets: { key: string; color: string; label: string }[] = [
  { key: "indigo", color: "#7b84ec", label: "靛蓝" },
  { key: "teal", color: "#58b3c4", label: "青" },
  { key: "green", color: "#6fb59a", label: "薄荷" },
  { key: "violet", color: "#a08fe0", label: "紫藤" },
  { key: "amber", color: "#d3a35e", label: "琥珀" },
  { key: "rose", color: "#d3859b", label: "蔷薇" },
];

const materials: { key: "frosted" | "liquid"; label: string; desc: string }[] = [
  { key: "frosted", label: "毛玻璃", desc: "奶霜质感，文字更清晰" },
  { key: "liquid", label: "液态玻璃", desc: "更薄更透，折射感强，更显壁纸" },
];

const wallKinds: { key: "none" | "gradient" | "image"; label: string }[] = [
  { key: "none", label: "无" },
  { key: "gradient", label: "柔光渐变" },
  { key: "image", label: "本地图片" },
];

const currentSkin = computed(() => activeSkinKey());

/** 取色器：预设色块选中时显示该预设的实际色值，拖动即切到自定义色 */
const pickerValue = computed({
  get: () => {
    const hit = accentPresets.find((a) => a.key === accent.value);
    return hit ? hit.color : accent.value;
  },
  set: (v: string) => (accent.value = v),
});

const accentLabel = computed(() =>
  accentIsPreset.value
    ? accentPresets.find((a) => a.key === accent.value)?.label ?? "预设"
    : "自定义"
);

function pickSkin(key: string) {
  const s = skins.find((x) => x.key === key);
  if (s) {
    applySkin(s);
    msg.value = `已应用皮肤「${s.label}」`;
    err.value = "";
  }
}

function randomAccent() {
  const list = accentPresets.map((a) => a.color);
  accent.value = list[Math.floor(Math.random() * list.length)];
  msg.value = "已随机换一个强调色";
}

async function pickWallpaper() {
  msg.value = "";
  err.value = "";
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择壁纸图片",
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    await invoke("wallpaper_set", { path: picked });
    wallKind.value = "image";
    emit("wallpaper-changed");
    msg.value = "壁纸已设置（只保存在本机数据目录）";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function clearWallpaper() {
  msg.value = "";
  err.value = "";
  try {
    await invoke("wallpaper_clear");
    if (wallImageUrl.value) URL.revokeObjectURL(wallImageUrl.value);
    wallImageUrl.value = null;
    wallKind.value = "gradient";
    msg.value = "已移除壁纸，回到柔光渐变";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function exportTheme() {
  msg.value = "";
  err.value = "";
  try {
    const picked = await save({
      title: "导出主题包",
      defaultPath: "tallymoment-theme.json",
      filters: [{ name: "主题包", extensions: ["json"] }],
    });
    if (!picked) return;
    const json = JSON.stringify(exportPack(), null, 2);
    await invoke("theme_export", { path: picked, json });
    msg.value = "主题包已导出（壁纸图片不包含在内）";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function importTheme() {
  msg.value = "";
  err.value = "";
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择主题包",
      filters: [{ name: "主题包", extensions: ["json"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    const txt = await invoke<string>("theme_import", { path: picked });
    const problem = importPack(JSON.parse(txt) as ThemePack);
    if (problem) {
      err.value = problem;
      return;
    }
    emit("wallpaper-changed");
    msg.value = "主题包已导入并应用";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function pickReminderBg() {
  msg.value = "";
  err.value = "";
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择全屏提醒背景图",
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    await invoke("reminder_bg_set", { path: picked });
    reminderBgSet.value = true;
    await loadReminderBg();
    msg.value = "全屏提醒背景已设置";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function clearReminderBg() {
  msg.value = "";
  err.value = "";
  try {
    await invoke("reminder_bg_clear");
    reminderBgSet.value = false;
    await loadReminderBg();
    msg.value = "已移除，回到默认柔光背景";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

/** 背景图组：每组最多 10 张；组名可改；轮换默认按顺序从左到右，开启后随机洗牌（轮内不重复） */
const bgOverview = ref<{ groups: { name: string; files: string[] }[]; active: number; rotate: boolean }>({
  groups: [],
  active: 0,
  rotate: false,
});
const bgThumbs = ref<{ path: string; mime: string; data: string; fit: string; align: string; zoom: number; scrim: number }[]>([]);
const renameValue = ref("");
/** 双击缩略图进入全图编辑：全屏预览里拖动调位置、实时看蒙版 */
const fullEdit = ref<null | { path: string; mime: string; data: string; fit: string; posX: number; posY: number; zoom: number; scrim: number }>(null);
const feEl = ref<HTMLElement | null>(null);
/** 仅左键按住时才跟随拖动（此前指针一移动图就跟着跑，没法用） */
let feDragging = false;

function parseAlignPct(a: string): [number, number] {
  const kw: Record<string, [number, number]> = {
    center: [50, 50], top: [50, 0], bottom: [50, 100], left: [0, 50], right: [100, 50],
    "left top": [0, 0], "right top": [100, 0], "left bottom": [0, 100], "right bottom": [100, 100],
  };
  if (kw[a]) return kw[a];
  const parts = a.split(" ");
  if (parts.length === 2 && parts.every((x) => x.endsWith("%"))) {
    const x = parseInt(parts[0]);
    const y = parseInt(parts[1]);
    if (!Number.isNaN(x) && !Number.isNaN(y)) return [x, y];
  }
  return [50, 50];
}

/** 显示器宽高比：编辑层用等比"虚拟屏幕"预览，保证与真实全屏所见一致 */
const monitorAR = ref(16 / 9);

async function loadMonitorAR() {
  try {
    const m = await currentMonitor();
    if (m && m.size.height > 0) monitorAR.value = m.size.width / m.size.height;
  } catch {
    /* 保底 16:9 */
  }
}

function openFullEdit(t: { path: string; mime: string; data: string; fit: string; align: string; zoom: number; scrim: number }) {
  const [x, y] = parseAlignPct(t.align);
  void loadMonitorAR();
  fullEdit.value = { path: t.path, mime: t.mime, data: t.data, fit: t.fit, posX: x, posY: y, zoom: t.zoom, scrim: t.scrim };
}

/** 拖拽基准：按下时的指针位置与位置基准值（拖多少、图动多少） */
const feDragBase = ref({ x: 0, y: 0, px: 50, py: 50 });

function feDown(e: MouseEvent) {
  if (e.button !== 0) return; // 只认左键：按住才拖
  e.preventDefault();
  feDragging = true;
  feDragBase.value = { x: e.clientX, y: e.clientY, px: fullEdit.value!.posX, py: fullEdit.value!.posY };
  // window 级监听：拖出预览区也照常跟踪
  window.addEventListener("mousemove", feMove);
  window.addEventListener("mouseup", feUp);
}

function feMove(e: MouseEvent) {
  if (!feDragging) return; // 松开/未按下：图不跟手
  const el = feEl.value;
  if (!el || !fullEdit.value) return;
  const rect = el.getBoundingClientRect();
  const dx = ((e.clientX - feDragBase.value.x) / rect.width) * 100;
  const dy = ((e.clientY - feDragBase.value.y) / rect.height) * 100;
  fullEdit.value.posX = Math.min(100, Math.max(0, Math.round(feDragBase.value.px + dx)));
  fullEdit.value.posY = Math.min(100, Math.max(0, Math.round(feDragBase.value.py + dy)));
}

function feUp() {
  if (!feDragging) return;
  feDragging = false;
  window.removeEventListener("mousemove", feMove);
  window.removeEventListener("mouseup", feUp);
  feSave();
}

function feSave() {
  const fe = fullEdit.value;
  if (!fe) return;
  invoke("reminder_bg_imgcfg_set", {
    path: fe.path,
    fit: fe.fit,
    align: `${fe.posX}% ${fe.posY}%`,
    zoom: fe.zoom,
    scrim: fe.scrim,
  }).catch((e) => (err.value = String(e).replace(/^.*Error: /, "")));
}

function feWheel(e: WheelEvent) {
  if (!fullEdit.value) return;
  e.preventDefault();
  const delta = e.deltaY < 0 ? 10 : -10;
  fullEdit.value.zoom = Math.min(300, Math.max(50, fullEdit.value.zoom + delta));
  feSave();
}

function feReset() {
  if (!fullEdit.value) return;
  fullEdit.value.fit = "cover";
  fullEdit.value.posX = 50;
  fullEdit.value.posY = 50;
  fullEdit.value.zoom = 100;
  fullEdit.value.scrim = 100;
  feSave();
}

function bumpFeScrim(delta: number) {
  if (!fullEdit.value) return;
  fullEdit.value.scrim = Math.min(100, Math.max(0, fullEdit.value.scrim + delta));
  feSave();
}

function closeFullEdit(save: boolean) {
  const fe = fullEdit.value;
  if (!fe) return;
  fullEdit.value = null;
  if (save) {
    invoke("reminder_bg_imgcfg_set", {
      path: fe.path,
      fit: fe.fit,
      align: `${fe.posX}% ${fe.posY}%`,
      zoom: fe.zoom,
      scrim: fe.scrim,
    })
      .then(() => loadReminderBg())
      .catch((e) => (err.value = String(e).replace(/^.*Error: /, "")));
  }
}

async function loadReminderBg() {
  try {
    bgOverview.value = await invoke("reminder_bg_overview");
    bgThumbs.value = await invoke<{ path: string; mime: string; data: string; fit: string; align: string; zoom: number; scrim: number }[]>("reminder_bg_thumbs");
    reminderBgSet.value = bgThumbs.value.length > 0;
  } catch {
    bgThumbs.value = [];
  }
}

async function switchBgGroup(i: number) {
  if (i === bgOverview.value.active) return;
  try {
    await invoke("reminder_bg_group_set_active", { index: i });
    await loadReminderBg();
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function addBgGroup() {
  try {
    await invoke("reminder_bg_group_add", { name: "" });
    await loadReminderBg();
    await switchBgGroup(bgOverview.value.groups.length - 1);
    msg.value = "已新建组，选择图片即可往里添加";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function renameBgGroup() {
  if (!renameValue.value.trim()) return;
  try {
    await invoke("reminder_bg_group_rename", {
      index: bgOverview.value.active,
      name: renameValue.value,
    });
    renameValue.value = "";
    await loadReminderBg();
    msg.value = "组名已更新";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function removeBgGroup(i: number) {
  try {
    await invoke("reminder_bg_group_remove", { index: i });
    await loadReminderBg();
    msg.value = "组已删除（不再被引用的图片已清理）";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function removeBg(path: string) {
  try {
    await invoke("reminder_bg_remove_file", { path: path });
    await loadReminderBg();
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function setReminderBgRotate(v: boolean) {
  try {
    await invoke("reminder_bg_rotate_set", { on: v });
    bgOverview.value.rotate = v;
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

void loadReminderBg();


/* ---------- 原子岛 ---------- */
const islandOn = ref(false);
const islandPos = ref("center");
const islandIdle = ref(true);
const islandOpacity = ref(100);
const islandHideDelay = ref(1);
const islandIdleWidth = ref(240);
const islandHideMode = ref("shrink");
const islandSnapReveal = ref(8);
const islandSnap = ref(false);
const islandSnapWake = ref("hover");
const islandClickThrough = ref(false);
const islandAlwaysTop = ref(true);
const islandCardOpen = ref(false); // 默认折叠：卡片太长，收起后只留标题行
const islandAccentMode = ref("endfield");
const POS_DEFS: Record<string, string> = {
  center: "顶部居中",
  left: "靠左",
  right: "靠右",
};
const ACCENT_MODE_DEFS: Record<string, string> = {
  endfield: "终末地",
  accent: "跟随强调色",
};
const MODULE_DEFS: Record<string, string> = {
  focus: "当前专注",
  next: "下个任务",
  done: "今日完成",
  clock: "时钟",
};
const islandModules = ref<string[]>(["focus", "next", "done"]);
const missingMods = computed(() =>
  Object.keys(MODULE_DEFS).filter((m) => !islandModules.value.includes(m))
);

async function saveModules() {
  try {
    await invoke("island_set_modules", { modules: islandModules.value });
    msg.value = "模块配置已更新";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

function moveMod(i: number, dir: -1 | 1) {
  const j = i + dir;
  if (j < 0 || j >= islandModules.value.length) return;
  const arr = [...islandModules.value];
  [arr[i], arr[j]] = [arr[j], arr[i]];
  islandModules.value = arr;
  void saveModules();
}

function removeMod(i: number) {
  islandModules.value = islandModules.value.filter((_, k) => k !== i);
  void saveModules();
}

function addMod(m: string) {
  islandModules.value = [...islandModules.value, m];
  void saveModules();
}
async function setIsland(v: boolean) {
  try {
    await invoke("island_set_enabled", { enabled: v });
    msg.value = v ? "原子岛已开启" : "原子岛已关闭";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function resetIslandPos() {
  try {
    await invoke("island_reset_pos");
    islandPos.value = "center";
    msg.value = "原子岛位置已重置";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function setIslandPos(m: string) {
  try {
    await invoke("island_set_pos_mode", { mode: m });
    islandPos.value = m;
    msg.value = "原子岛位置已更新";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function setIslandIdle(v: boolean) {
  try {
    await invoke("island_set_idle_enabled", { enabled: v });
    msg.value = v ? "无悬停时缩小已开启" : "无悬停时缩小已关闭";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function setIslandOpacity(delta: number) {
  try {
    const v = Math.min(100, Math.max(40, islandOpacity.value + delta));
    await invoke("island_set_opacity", { v });
    islandOpacity.value = v;
    msg.value = "原子岛透明度已更新";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function setIslandBehavior(
  part: "delay" | "width" | "reveal",
  delta: number
) {
  try {
    if (part === "delay") {
      const v = Math.min(30, Math.max(0, islandHideDelay.value + delta));
      await invoke("island_set_behavior", { hideDelaySec: v });
      islandHideDelay.value = v;
    } else if (part === "width") {
      const v = Math.min(360, Math.max(120, islandIdleWidth.value + delta));
      await invoke("island_set_behavior", { idleWidth: v });
      islandIdleWidth.value = v;
    } else {
      const v = Math.min(24, Math.max(4, islandSnapReveal.value + delta));
      await invoke("island_set_behavior", { snapReveal: v });
      islandSnapReveal.value = v;
    }
    msg.value = "原子岛行为已更新";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function setIslandSnap(v: boolean) {
  try {
    await invoke("island_set_snap_enabled", { on: v });
    islandSnap.value = v;
    msg.value = v ? "靠边吸附已开启" : "靠边吸附已关闭";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function setIslandSnapWake(m: string) {
  try {
    await invoke("island_set_snap_wake", { mode: m });
    islandSnapWake.value = m;
    msg.value = m === "hover" ? "靠近露出条即自动弹出" : "点击露出条才弹出";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function setIslandClickThrough(v: boolean) {
  try {
    await invoke("island_set_click_through", { on: v });
    islandClickThrough.value = v;
    msg.value = v
      ? "鼠标穿透已开启（岛为纯展示，回本页关闭）"
      : "鼠标穿透已关闭";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function setIslandAlwaysTop(v: boolean) {
  try {
    await invoke("island_set_always_top", { on: v });
    islandAlwaysTop.value = v;
    msg.value = v ? "原子岛已置顶" : "原子岛已取消置顶";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function setIslandAccentMode(m: string) {
  try {
    await invoke("island_set_accent_mode", { mode: m });
    islandAccentMode.value = m;
    msg.value = m === "endfield" ? "已切换终末地配色" : "已跟随主程序强调色";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
onMounted(async () => {
  try {
    const enabled = await invoke<boolean>("island_get_enabled");
    islandOn.value = enabled;
    const mods = await invoke<string[]>("island_get_modules");
    if (mods.length) islandModules.value = mods;
    islandPos.value = await invoke<string>("island_get_pos_mode");
    islandIdle.value = await invoke<boolean>("island_get_idle_enabled");
    const d = await invoke<{ opacity: number; accentMode: string; hideDelaySec: number; idleWidth: number; hideMode: string; snapReveal: number; snapEnabled: boolean; clickThrough: boolean; alwaysTop: boolean; snapWake: string }>("island_data");
    islandOpacity.value = Math.round(d.opacity);
    islandAccentMode.value = d.accentMode;
    islandHideDelay.value = d.hideDelaySec;
    islandIdleWidth.value = d.idleWidth;
    islandHideMode.value = d.hideMode;
    islandSnapReveal.value = d.snapReveal;
    islandSnap.value = d.snapEnabled;
    islandClickThrough.value = d.clickThrough;
    islandAlwaysTop.value = d.alwaysTop;
    islandSnapWake.value = d.snapWake === "click" ? "click" : "hover";
  } catch {
    islandOn.value = false;
  }
});
</script>

<template>
  <div class="page">
    <header class="phead">
      <h1>个性化</h1>
      <span class="sub">皮肤预设 · 壁纸 · 玻璃材质 · 透明度 · 动效</span>
    </header>

    <!-- 显示（原在设置页，按用户要求移到这里，放在皮肤预设之前） -->
    <div class="glass-card card">
      <h2>显示</h2>
      <div class="row col">
        <div class="rlabel">
          <p class="rt">应用配色</p>
          <p class="rd">
            随机色更易区分；跟随强调色更整体；按图标取色＝从程序图标提取主色；显示应用图标＝直接显示 exe 图标
          </p>
        </div>
        <div class="seg grid2">
          <button
            class="seg-item"
            :class="{ active: appColorMode === 'random' }"
            @click="appColorMode = 'random'"
          >
            随机色
          </button>
          <button
            class="seg-item"
            :class="{ active: appColorMode === 'accent' }"
            @click="appColorMode = 'accent'"
          >
            跟随强调色
          </button>
          <button
            class="seg-item"
            :class="{ active: appColorMode === 'iconColor' }"
            @click="appColorMode = 'iconColor'"
          >
            按图标取色
          </button>
          <button
            class="seg-item"
            :class="{ active: appColorMode === 'icon' }"
            @click="appColorMode = 'icon'"
          >
            显示应用图标
          </button>
        </div>
      </div>
      <SettingSlider
        v-model="appsTopN"
        label="应用排行显示条数"
        desc="5 ~ 20 条，默认 10 条"
        :min="5"
        :max="20"
        suffix=" 条"
      />
    </div>

    <!-- 原子岛（常驻胶囊） -->
    <div class="glass-card card">
      <h2 class="card-h">
        原子岛
        <button class="mini fold" @click="islandCardOpen = !islandCardOpen">
          {{ islandCardOpen ? "收起 ▲" : "展开 ▼" }}
        </button>
      </h2>
      <div v-show="islandCardOpen">
      <div class="row">
        <span>启用常驻胶囊</span>
        <NSwitch v-model:value="islandOn" size="small" @update:value="setIsland" />
      </div>
      <div class="row col">
        <div class="rlabel">
          <p class="rt">预设</p>
          <p class="rd">终末地＝超充黄绿深色 HUD；跟随强调色＝用〈强调色〉里设置的的颜色</p>
        </div>
        <div class="seg grid2">
          <button
            v-for="(label, key) in ACCENT_MODE_DEFS"
            :key="key"
            class="seg-item"
            :class="{ active: islandAccentMode === key }"
            @click="setIslandAccentMode(key)"
          >
            {{ label }}
          </button>
        </div>
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">透明度</p>
          <p class="rd">100 = 不透明深底；调低后岛变半透明（40~100）</p>
        </div>
        <div class="stepper">
          <button class="mini" @click="setIslandOpacity(-5)">−</button>
          <span class="sval">{{ islandOpacity }} %</span>
          <button class="mini" @click="setIslandOpacity(5)">+</button>
        </div>
      </div>
      <div class="row col">
        <div class="rlabel">
          <p class="rt">位置</p>
          <p class="rd">吸附屏幕顶部；也可直接拖动胶囊，拖动后按你放的位置停留（自定义）</p>
        </div>
        <div class="seg grid3">
          <button
            v-for="(label, key) in POS_DEFS"
            :key="key"
            class="seg-item"
            :class="{ active: islandPos === key }"
            @click="setIslandPos(key)"
          >
            {{ label }}
          </button>
        </div>
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">自动隐藏</p>
          <p class="rd">鼠标移开后隐藏；按住左键拖动可挪位置，点击胶囊展开</p>
        </div>
        <NSwitch v-model:value="islandIdle" size="small" @update:value="setIslandIdle" />
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">窗口置顶</p>
          <p class="rd">开＝岛始终浮在最上层；关＝可以被其他窗口挡住</p>
        </div>
        <NSwitch v-model:value="islandAlwaysTop" size="small" @update:value="setIslandAlwaysTop" />
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">鼠标穿透</p>
          <p class="rd">开＝岛变纯展示，点击全部穿到下层应用且不再响应悬停；需回本页关闭</p>
        </div>
        <NSwitch v-model:value="islandClickThrough" size="small" @update:value="setIslandClickThrough" />
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">靠边吸附</p>
          <p class="rd">开启后隐藏时自动贴向最近的屏幕边（上/下/左/右），只露一小条，碰到露出的边即滑回；离边太远则照常缩小胶囊</p>
        </div>
        <NSwitch v-model:value="islandSnap" size="small" @update:value="setIslandSnap" />
      </div>
      <div class="row col" v-if="islandSnap">
        <div class="rlabel">
          <p class="rt">唤回方式</p>
          <p class="rd">靠近自动弹出＝光标靠近露出的边即滑回；点击弹出＝碰到不打扰，点一下露出的边才滑回</p>
        </div>
        <div class="seg grid2">
          <button
            class="seg-item"
            :class="{ active: islandSnapWake === 'hover' }"
            @click="setIslandSnapWake('hover')"
          >
            靠近自动弹出
          </button>
          <button
            class="seg-item"
            :class="{ active: islandSnapWake === 'click' }"
            @click="setIslandSnapWake('click')"
          >
            点击弹出
          </button>
        </div>
      </div>
      <div class="row" v-if="islandSnap">
        <div class="rlabel">
          <p class="rt">吸附露出高度</p>
          <p class="rd">吸附后露在屏幕边缘的那条边的宽度/高度（4~24px）</p>
        </div>
        <div class="stepper">
          <button class="mini" @click="setIslandBehavior('reveal', -2)">−</button>
          <span class="sval">{{ islandSnapReveal }} px</span>
          <button class="mini" @click="setIslandBehavior('reveal', 2)">+</button>
        </div>
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">隐藏延迟</p>
          <p class="rd">鼠标离开后等几秒再缩小（0 = 立即）</p>
        </div>
        <div class="stepper">
          <button class="mini" @click="setIslandBehavior('delay', -1)">−</button>
          <span class="sval">{{ islandHideDelay }} 秒</span>
          <button class="mini" @click="setIslandBehavior('delay', 1)">+</button>
        </div>
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">隐藏后宽度</p>
          <p class="rd">缩小态胶囊的宽度（120~360，按你喜欢调）</p>
        </div>
        <div class="stepper">
          <button class="mini" @click="setIslandBehavior('width', -20)">−</button>
          <span class="sval">{{ islandIdleWidth }} px</span>
          <button class="mini" @click="setIslandBehavior('width', 20)">+</button>
        </div>
      </div>
      <p class="rd more">
        悬停显示完整胶囊；点击胶囊本体展开待办列表并可直接勾掉，再点一下收起；
        「打开设置」跳回这里。常驻约多占 40~80MB 内存（一个 WebView 窗口）。
      </p>
      <div class="mods">
        <div v-for="(m, i) in islandModules" :key="m" class="modrow2">
          <span class="mname">{{ MODULE_DEFS[m] || m }}</span>
          <button class="mini" :disabled="i === 0" title="上移" @click="moveMod(i, -1)">↑</button>
          <button class="mini" :disabled="i === islandModules.length - 1" title="下移" @click="moveMod(i, 1)">↓</button>
          <button class="mini" title="从胶囊移除" @click="removeMod(i)">移除</button>
        </div>
        <div v-if="missingMods.length" class="modrow2">
          <span class="mname faint">添加模块：</span>
          <button v-for="m in missingMods" :key="m" class="mini add" @click="addMod(m)">
            + {{ MODULE_DEFS[m] }}
          </button>
        </div>
      </div>
      <div class="acts" style="display: flex; gap: 8px">
        <button class="btn" @click="resetIslandPos">位置重置（回屏幕顶部居中）</button>
      </div>
      <p v-if="msg" class="ok">{{ msg }}</p>
      </div>
    </div>

    <!-- 皮肤预设 -->
    <div class="glass-card card">
      <h2>皮肤预设</h2>
      <div class="skins">
        <button
          v-for="s in skins"
          :key="s.key"
          class="skin"
          :class="{ active: currentSkin === s.key }"
          :title="s.label"
          @click="pickSkin(s.key)"
        >
          <span class="preview" :style="{ background: s.swatch[0] }">
            <span class="pv-card" :style="{ background: s.swatch[1] }">
              <span class="pv-bar" :style="{ background: s.swatch[2] }"></span>
              <span class="pv-bar short" :style="{ background: s.swatch[2], opacity: 0.6 }"></span>
            </span>
          </span>
          <span class="sname">{{ s.label }}</span>
          <span v-if="currentSkin === s.key" class="tick"><Icon name="checklist" :size="12" /></span>
        </button>
      </div>
      <p class="rd more">预设会一次性设定主题、强调色、材质与透明度；之后你仍然可以单独微调任何一项。</p>
    </div>

    <!-- 强调色 -->
    <div class="glass-card card">
      <h2>强调色</h2>
      <div class="accents">
        <button
          v-for="a in accentPresets"
          :key="a.key"
          class="swatch"
          :class="{ active: accent === a.key }"
          :style="{ background: a.color }"
          :title="a.label"
          @click="accent = a.key"
        ></button>
        <label class="picker" :title="`自定义颜色（当前：${accentLabel}）`">
          <input v-model="pickerValue" type="color" />
        </label>
        <button class="ghost" @click="randomAccent">随机</button>
        <button class="ghost" @click="accent = 'indigo'">恢复默认</button>
        <span class="cur">{{ accentLabel }}</span>
      </div>
    </div>

    <!-- 主题与壁纸 -->
    <div class="glass-card card">
      <h2>主题与壁纸</h2>
      <div class="row">
        <div class="rlabel">
          <p class="rt">明暗主题</p>
          <p class="rd">浅色主题下图表与图标也已适配</p>
        </div>
        <div class="seg">
          <button
            v-for="t in themes"
            :key="t.key"
            class="seg-item"
            :class="{ active: themePref === t.key }"
            @click="themePref = t.key"
          >
            <Icon :name="t.icon" :size="15" />
            <span>{{ t.label }}</span>
          </button>
        </div>
      </div>

      <div class="row">
        <div class="rlabel">
          <p class="rt">壁纸</p>
          <p class="rd">玻璃效果需要有"东西"可糊，柔光渐变是最省性能的选择</p>
        </div>
        <div class="seg">
          <button
            v-for="w in wallKinds"
            :key="w.key"
            class="seg-item"
            :class="{ active: wallKind === w.key }"
            @click="wallKind = w.key"
          >
            {{ w.label }}
          </button>
        </div>
      </div>

      <div class="row">
        <div class="rlabel">
          <p class="rt">本地图片</p>
          <p class="rd">从本机选择一张图作为底衬（只存本机数据目录，不上传）</p>
        </div>
        <div class="acts">
          <button class="ghost" @click="pickWallpaper">选择图片</button>
          <button class="ghost danger" @click="clearWallpaper">移除图片</button>
        </div>
      </div>
    </div>

    <!-- 玻璃效果：所有"有多透"的控制收在同一组 -->
    <div class="glass-card card">
      <h2>玻璃效果</h2>

      <div class="row">
        <div class="rlabel">
          <p class="rt">毛玻璃总开关</p>
          <p class="rd">关闭后界面变为不透明实底（低配设备可关）</p>
        </div>
        <NSwitch :value="glass" @update:value="(v: boolean) => (glass = v)" />
      </div>

      <div class="materials">
        <button
          v-for="m in materials"
          :key="m.key"
          class="mat"
          :class="{ active: material === m.key }"
          @click="material = m.key"
        >
          <span class="mat-pv" :class="m.key"></span>
          <span class="sname">{{ m.label }}</span>
          <span class="sdesc">{{ m.desc }}</span>
        </button>
      </div>

    </div>

    <!-- 背景 -->
    <div class="glass-card card">
      <h2>背景</h2>
      <SettingSlider v-model="bgBlur" label="背景模糊" desc="0 ~ 60px：柔光/壁纸的朦胧程度" :min="0" :max="60" suffix="px" />
      <SettingSlider v-model="bgAlpha" label="背景不透明度" desc="30% ~ 100%，越低桌面越透" :min="30" :max="100" suffix="%" />
    </div>

    <!-- 侧边栏 -->
    <div class="glass-card card">
      <h2>侧边栏</h2>
      <SettingSlider v-model="sideBlur" label="侧边栏亚克力模糊" desc="0 ~ 60px" :min="0" :max="60" suffix="px" />
      <SettingSlider v-model="sideAlpha" label="侧边栏不透明度" desc="0% ~ 100%" :min="0" :max="100" suffix="%" />
    </div>

    <!-- 卡片 -->
    <div class="glass-card card">
      <h2>卡片</h2>
      <SettingSlider v-model="cardBlur" label="卡片亚克力模糊" desc="0 ~ 60px" :min="0" :max="60" suffix="px" />
      <SettingSlider v-model="cardAlpha" label="卡片不透明度" desc="50% ~ 100%" :min="50" :max="100" suffix="%" />
      <SettingSlider v-model="cardShadow" label="卡片阴影强度" desc="0 = 无阴影（扁平）；1 = 标准；越大浮起感越强" :min="0" :max="2" :step="0.1" suffix="×" />
    </div>

    <!-- 全屏提醒背景 -->
    <div class="glass-card card">
      <h2>全屏提醒背景</h2>
      <div class="row">
        <div class="rlabel">
          <p class="rt">自定义图片</p>
          <p class="rd">全屏休息提醒时铺满屏幕；不设则用默认的柔光背景。最近 5 张自动留作历史</p>
        </div>
        <div class="acts">
          <button class="ghost" @click="pickReminderBg">选择图片</button>
          <button class="ghost danger" @click="clearReminderBg">移除</button>
        </div>
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">图片组</p>
          <p class="rd">点击切换组（当前组高亮）；每组最多 10 张，互相独立；组旁 ✕ 删除该组</p>
        </div>
      </div>
      <div class="bggroups">
        <span
          v-for="(g, i) in bgOverview.groups"
          :key="i"
          class="bggroup"
          :class="{ on: i === bgOverview.active }"
        >
          <button class="gname" @click="switchBgGroup(i)">{{ g.name }}</button>
          <button
            v-if="bgOverview.groups.length > 1"
            class="gdel"
            title="删除该组（组内图片一并清理）"
            @click="removeBgGroup(i)"
          >✕</button>
        </span>
        <button class="bggroup add" @click="addBgGroup">＋ 新组</button>
        <input
          v-model="renameValue"
          class="rnin"
          placeholder="改当前组名"
          @keyup.enter="renameBgGroup"
        />
        <button class="mini" title="应用改名" @click="renameBgGroup">改</button>
      </div>
      <div v-if="bgThumbs.length" class="bgthumbs">
        <div
          v-for="(t, i) in bgThumbs"
          :key="t.path"
          class="bgthumb"
          :class="{ cur: i === 0 }"
          :title="i === 0 ? '当前使用 · 双击进入全图编辑' : '双击进入全图编辑'"
          @dblclick="openFullEdit(t)"
        >
          <img :src="`data:${t.mime};base64,${t.data}`" alt="" />
          <button class="tremove" title="从本组移除" @click.stop="removeBg(t.path)">✕</button>
          <span v-if="i === 0" class="tag">当前</span>
        </div>
      </div>
      <p v-if="bgThumbs.length" class="rd more">双击缩略图进入全图编辑：拖动调整位置、实时预览蒙版浓淡。</p>
      <p v-else class="rd more">当前组还没有图片，点上面「选择图片」添加。</p>

      <div class="row">
        <div class="rlabel">
          <p class="rt">轮换方式</p>
          <p class="rd">关＝按顺序从左到右循环（默认）；开＝随机洗牌，一轮内不重复，一轮结束自动重洗</p>
        </div>
        <NSwitch
          :value="bgOverview.rotate"
          size="small"
          @update:value="(v: boolean) => setReminderBgRotate(v)"
        />
      </div>
      <p v-if="reminderBgSet" class="ok">已设置（存本机数据目录，不上传）</p>
    </div>

    <!-- 动效 -->
    <div class="glass-card card">
      <h2>动效</h2>
      <SettingSlider
        v-model="motion"
        label="动效强度"
        desc="0 = 关闭动画；1 = 标准；越大悬停放大与上浮越明显"
        :min="0"
        :max="1.6"
        :step="0.1"
        suffix="×"
      />
      <p class="rd more">
        现在把鼠标移到任意卡片上试试：卡片会上浮并轻微放大、描边变成强调色。强度拉到 0 就是完全静态。
      </p>
    </div>

    <!-- 主题包 -->
    <div class="glass-card card">
      <h2>主题包（本机文件）</h2>
      <div class="acts">
        <button class="ghost" @click="exportTheme">导出主题包</button>
        <button class="ghost" @click="importTheme">导入主题包</button>
      </div>
      <p class="rd more">
        导出为一份 JSON（皮肤/材质/透明度/动效参数），可在别的机器上导入复现同一套观感。壁纸图片不包含在内。
      </p>
    </div>

    <p v-if="msg" class="ok">{{ msg }}</p>
    <p v-if="err" class="err">{{ err }}</p>
    <!-- 全图编辑层：双击缩略图进入，拖动调位置，实时预览蒙版。
         stage = 与显示器等比的"虚拟屏幕"，保证窗口模式下的预览与真实全屏一致。
         （Teleport 渲染到 body，但节点必须留在根 div 内保持单根，否则页面的 v-show 隐藏失效） -->
    <Teleport to="body">
    <div v-if="fullEdit" class="fulledit">
      <div
        class="fe-stage"
        :style="{ aspectRatio: String(monitorAR), width: `min(96vw, ${88 * monitorAR}vh)` }"
      >
        <img
          ref="feEl"
          class="fe-img"
          :src="`data:${fullEdit.mime};base64,${fullEdit.data}`"
          alt=""
          :style="{
            objectFit: fullEdit.fit === 'contain' ? 'contain' : 'cover',
            objectPosition: `${fullEdit.posX}% ${fullEdit.posY}%`,
            transform: `scale(${fullEdit.zoom / 100})`,
          }"
          draggable="false"
          @mousedown="feDown"
          @wheel.prevent="feWheel"
        />
        <div class="fe-scrim" :style="{ opacity: String(fullEdit.scrim / 100) }"></div>
        <p class="fe-tip">左键按住拖动调位置 · 滚轮缩放 · 蒙版实时预览</p>
      </div>
      <div class="fe-bar">
        <button class="febtn" @click="fullEdit.fit = fullEdit.fit === 'cover' ? 'contain' : 'cover'">
          {{ fullEdit.fit === "cover" ? "铺满裁剪" : "完整显示" }}
        </button>
        <span class="fsep"></span>
        <span class="fl">缩放</span>
        <span class="fnum">{{ fullEdit.zoom }}%</span>
        <span class="fl">蒙版</span>
        <button class="febtn" @click="bumpFeScrim(-10)">−</button>
        <span class="fnum acc">{{ fullEdit.scrim }}%</span>
        <button class="febtn" @click="bumpFeScrim(10)">+</button>
        <button class="febtn" title="恢复默认（铺满 / 居中 / 100% / 蒙版100）" @click="feReset">恢复默认</button>
        <span class="fsep"></span>
        <button class="febtn primary" @click="closeFullEdit(true)">保存并退出</button>
        <button class="febtn" @click="closeFullEdit(false)">取消</button>
      </div>
    </div>
  </Teleport>
  </div>
</template>

<style scoped>
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

.card h2 {
  margin: 0 0 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

/* 可折叠卡片标题：右上角收起/展开 */
.card h2.card-h {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.card h2.card-h .fold {
  flex: none;
  font-size: 11px;
  color: var(--text-muted);
  background: none;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
  border-radius: 999px;
  padding: 2px 10px;
  cursor: pointer;
}
.card h2.card-h .fold:hover {
  color: var(--text);
  border-color: var(--text-muted);
}

/* 皮肤预设 */
.skins {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
  gap: 10px;
}

.skin {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 6px;
  align-items: stretch;
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-md);
  padding: 8px;
  cursor: pointer;
  font-family: inherit;
  transition: transform var(--dur), border-color var(--dur), box-shadow var(--dur);
}

.skin:hover {
  transform: translateY(calc(-2px * var(--motion))) scale(calc(1 + 0.02 * var(--motion)));
  border-color: var(--accent-border);
  box-shadow: var(--shadow-soft);
}

.skin.active {
  border-color: var(--accent-border);
  box-shadow: 0 0 0 2px var(--accent-soft) inset;
}

.preview {
  display: block;
  height: 46px;
  border-radius: var(--r-sm);
  padding: 5px;
}

.pv-card {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 4px;
  height: 100%;
  border-radius: 5px;
  padding: 0 6px;
}

.pv-bar {
  display: block;
  height: 3px;
  border-radius: 2px;
}

.pv-bar.short {
  width: 60%;
}

.sname {
  font-size: 12px;
  color: var(--text);
}

.sdesc {
  font-size: 11px;
  color: var(--text-muted);
}

.tick {
  position: absolute;
  top: 6px;
  right: 6px;
  color: var(--accent-text);
  background: var(--accent-soft);
  border-radius: var(--r-full);
  padding: 2px;
  display: inline-flex;
}

/* 强调色 */
.accents {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.swatch {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  transition: transform var(--dur), box-shadow var(--dur);
}

.swatch:hover {
  transform: scale(calc(1 + 0.12 * var(--motion)));
}

.swatch.active {
  border-color: var(--text);
  box-shadow: 0 0 0 3px var(--accent-soft);
}

.picker {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  border: 1px dashed var(--border-strong);
  overflow: hidden;
  display: inline-flex;
  cursor: pointer;
}

.picker input {
  width: 200%;
  height: 200%;
  margin: -25%;
  border: 0;
  padding: 0;
  background: transparent;
  cursor: pointer;
}

.cur {
  font-size: 11.5px;
  color: var(--text-faint);
}

/* 全屏提醒背景：图片组 */
.bggroups {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin: 4px 0 10px;
}
.bggroup {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 999px;
  border: 1px solid rgba(128, 128, 128, 0.35);
  background: none;
  color: var(--text, #ddd);
}
.bggroup.on {
  border-color: var(--accent, #7b84ec);
}
.bggroup .gname {
  border: 0;
  background: none;
  font-size: 12px;
  color: inherit;
  cursor: pointer;
  padding: 0;
}
.bggroup.on .gname {
  color: var(--accent, #7b84ec);
  font-weight: 600;
}
.bggroup .gdel {
  border: 0;
  background: none;
  font-size: 10px;
  color: var(--text-faint, #999);
  cursor: pointer;
  padding: 0 2px;
}
.bggroup .gdel:hover {
  color: var(--danger, #e5484d);
}
.bggroup.add {
  border-style: dashed;
  opacity: 0.8;
  cursor: pointer;
}
.rnin {
  font-size: 11px;
  padding: 3px 8px;
  width: 130px;
  border: 1px dashed rgba(128, 128, 128, 0.35);
  border-radius: 6px;
  background: none;
  color: var(--text, #ddd);
}
.bgedit {
  margin: 8px 0 4px;
  padding: 10px 12px;
  border: 1px solid rgba(128, 128, 128, 0.25);
  border-radius: 10px;
}
.bgedit-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.bgedit-row .l {
  font-size: 11px;
  color: var(--text-muted, #888);
}
.esel2 {
  font-size: 12px;
  background: none;
  color: var(--text, #ddd);
  border: 1px solid rgba(128, 128, 128, 0.35);
  border-radius: 6px;
  padding: 3px 6px;
}
.bgthumb.sel {
  border-color: var(--accent, #7b84ec);
  box-shadow: 0 0 0 1px var(--accent, #7b84ec);
}

/* 全图编辑层（双击缩略图进入） */
.fulledit {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: #080a10;
  display: flex;
  align-items: center;
  justify-content: center;
}
/* 虚拟屏幕：与显示器等比，预览即所得 */
.fe-stage {
  position: relative;
  width: min(96vw, calc(88vh * var(--ar, 1.7778)));
  max-height: 88vh;
  overflow: hidden;
  border-radius: 6px;
  box-shadow: 0 12px 60px rgba(0, 0, 0, 0.6);
}
.fe-img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  cursor: grab;
  touch-action: none;
}
.fe-img:active {
  cursor: grabbing;
}
.fe-scrim {
  position: absolute;
  inset: 0;
  background: linear-gradient(180deg, rgba(8, 10, 16, 0.62), rgba(8, 10, 16, 0.78));
  pointer-events: none;
}
.fe-bar {
  position: absolute;
  top: 18px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 14px;
  white-space: nowrap;
  background: rgba(10, 12, 18, 0.85);
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 999px;
  padding: 10px 26px;
  z-index: 2;
  max-width: none;
}
.fe-bar * {
  white-space: nowrap;
}
.febtn {
  flex: none;
  font-size: 12px;
  padding: 5px 14px;
  border-radius: 999px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  background: rgba(255, 255, 255, 0.06);
  color: #eef1e9;
  cursor: pointer;
}
.febtn:hover {
  border-color: rgba(255, 255, 255, 0.45);
}
.febtn.primary {
  background: var(--accent, #7b84ec);
  border-color: var(--accent, #7b84ec);
  color: #10131a;
  font-weight: 600;
}
.fsep {
  width: 1px;
  height: 18px;
  background: rgba(255, 255, 255, 0.16);
}
.fl {
  font-size: 11px;
  color: #9aa3ad;
}
.fnum {
  font-size: 14px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: #ffffff;
  min-width: 46px;
  text-align: center;
}
.fnum.acc {
  color: var(--accent, #7b84ec);
}
.fe-tip {
  position: absolute;
  bottom: 22px;
  left: 50%;
  transform: translateX(-50%);
  margin: 0;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.55);
  white-space: nowrap;
}
.bgthumb .tremove {
  position: absolute;
  top: 3px;
  right: 3px;
  width: 18px;
  height: 18px;
  line-height: 1;
  font-size: 10px;
  border: 0;
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  cursor: pointer;
}

/* 全屏提醒背景：历史缩略图 */
.bgthumbs {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin: 4px 0 10px;
}
.bgthumb {
  position: relative;
  width: 96px;
  height: 60px;
  border-radius: 8px;
  overflow: hidden;
  border: 2px solid rgba(128, 128, 128, 0.35);
}
.bgthumb.cur {
  border-color: var(--accent, #7b84ec);
}
.bgthumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.bgthumb .tag {
  position: absolute;
  left: 4px;
  bottom: 4px;
  font-size: 10px;
  padding: 0 6px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
}

.ghost {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-sm);
  font-size: 12px;
  font-family: inherit;
  padding: 6px 12px;
  cursor: pointer;
  transition: background var(--dur), color var(--dur);
}

.ghost:hover {
  background: var(--surface-hover);
  color: var(--text);
}

.ghost.danger:hover {
  color: var(--danger);
  border-color: var(--danger);
}

.acts {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 8px 0;
}

.rlabel .rt {
  margin: 0 0 2px;
  font-size: 13.5px;
  color: var(--text);
}

.rlabel .rd {
  margin: 0;
  font-size: 11.5px;
  color: var(--text-muted);
}

.seg {
  display: inline-flex;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  padding: 4px;
  background: var(--surface);
  flex-wrap: wrap;
}

/* 四个配色模式：上两个、下两个，等宽对齐（窗口模式下也不歪） */
.seg.grid2 {
  display: grid;
  grid-template-columns: repeat(2, minmax(140px, 1fr));
  gap: 6px;
}

/* 原子岛位置三选：一行等宽 */
.seg.grid3 {
  display: grid;
  grid-template-columns: repeat(3, minmax(110px, 1fr));
  gap: 6px;
}

.seg.grid2 .seg-item,
.seg.grid3 .seg-item {
  justify-content: center;
}

.seg-item {
  display: flex;
  align-items: center;
  gap: 6px;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 12.5px;
  font-family: inherit;
  padding: 6px 12px;
  border-radius: var(--r-sm);
  cursor: pointer;
}

.seg-item.active {
  color: var(--accent-text);
  background: var(--accent-soft);
}

/* 材质双卡 */
.materials {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin: 6px 0 4px;
}

.mat {
  display: flex;
  flex-direction: column;
  gap: 4px;
  text-align: left;
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-md);
  padding: 10px;
  cursor: pointer;
  font-family: inherit;
  transition: transform var(--dur), border-color var(--dur);
}

.mat:hover {
  transform: translateY(calc(-2px * var(--motion)));
}

.mat.active {
  border-color: var(--accent-border);
  box-shadow: 0 0 0 2px var(--accent-soft) inset;
}

.mat-pv {
  display: block;
  height: 42px;
  border-radius: var(--r-sm);
  background-image: linear-gradient(
    100deg,
    #ff9966,
    #ffd166,
    #6fb59a,
    #58b3c4,
    #a08fe0
  );
}

.mat-pv.frosted {
  backdrop-filter: blur(8px) saturate(1.3) brightness(1.08);
  opacity: 0.85;
}

.mat-pv.liquid {
  backdrop-filter: blur(2px) saturate(1.8) brightness(1.05) contrast(1.04);
  border: 1px solid color-mix(in srgb, var(--text) 30%, transparent);
}

.rd.more {
  margin: 6px 0 0;
  font-size: 11.5px;
  line-height: 1.7;
  color: var(--text-faint);
}

.ok {
  margin: 0;
  font-size: 12px;
  color: var(--good);
}

.mods {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 8px 0;
}

.modrow2 {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mname {
  flex: 1;
  font-size: 12.5px;
  color: var(--text);
}

.mname.faint {
  color: var(--text-faint);
}

.stepper {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  padding: 3px 8px;
  background: var(--surface);
}

.sval {
  min-width: 52px;
  text-align: center;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.mini {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-sm);
  font-size: 11px;
  font-family: inherit;
  padding: 3px 10px;
  cursor: pointer;
  transition: color var(--dur), border-color var(--dur);
}

.mini:hover {
  color: var(--accent-text);
  border-color: var(--accent-border);
}

.mini:disabled {
  opacity: 0.4;
  cursor: default;
}

.mini.add {
  color: var(--accent-text);
  border-color: var(--accent-border);
}

.err {
  margin: 0;
  font-size: 12px;
  color: var(--danger);
}
</style>
