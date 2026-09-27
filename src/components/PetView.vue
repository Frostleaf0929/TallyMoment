<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { fmtDuration } from "../lib/format";

// 内置原版 BongoCat 分层素材，或已导入的 Mver 模型：分层渲染 + 按键矩阵映射
interface PetSettingsView {
  scale: number;
  opacity: number;
  mirror: boolean;
  activeModel: string;
  activeModelDir: string | null;
  mode: "keyboard" | "gamepad" | "standard";
}

interface AssetFile {
  rel: string;
  mime: string;
  data: string;
}

const settings = ref<PetSettingsView | null>(null);
const seconds = ref(0);
const keys = ref(0);
const clicks = ref(0);
const loadErr = ref("");

const layerBg = ref<string | null>(null);
const layerCat = ref<string | null>(null);
const layerFace = ref<string | null>(null);
const layerLeft = ref<string | null>(null);
const layerRight = ref<string | null>(null);

/** 自定义模型素材：rel -> Blob URL（Rust 直读，不走 asset 协议） */
const assetUrls = new Map<string, string>();

// 按键矩阵（行号 = 帧编号，行内 = VK 码）
let leftMatrix: number[][] = [];
let rightMatrix: number[][] = [];
// 抬爪帧的文件名（keyboard 模式为 leftup/rightup；standard 模式用 0 号帧）
let leftUpName = "leftup";
let rightUpName = "rightup";
// 当前左右手帧的文件名前缀（rel 路径，用于换帧）
let leftPrefix: string | null = null;
let rightPrefix: string | null = null;

let unlistenInput: UnlistenFn | undefined;
let unlistenSettings: UnlistenFn | undefined;
let revertTimer: number | undefined;
let pollTimer: number | undefined;
let alternateSide = false;

async function refresh() {
  try {
    const r = await invoke<{ totalSeconds: number }>("today_report");
    seconds.value = r.totalSeconds;
    const [k, c] = await invoke<[number, number]>("pet_stats");
    keys.value = k;
    clicks.value = c;
  } catch {
    /* 后端未就绪时静默 */
  }
}

async function loadSettings() {
  try {
    const s = await invoke<PetSettingsView>("pet_settings_get");
    settings.value = s;
    await buildModel(s.activeModel, s.mode);
  } catch (e) {
    loadErr.value = String(e);
  }
}

/** base64 -> Blob URL */
function toBlobUrl(asset: AssetFile): string {
  const bin = atob(asset.data);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return URL.createObjectURL(new Blob([bytes], { type: asset.mime }));
}

function revokeAssets() {
  for (const url of assetUrls.values()) URL.revokeObjectURL(url);
  assetUrls.clear();
}

/** 内置素材：随前端打包，直接静态路径 */
function buildBuiltin() {
  assetUrls.clear();
  layerBg.value = "/pet/kb/bg.png";
  layerCat.value = "/pet/kb/cat.png";
  layerFace.value = "/pet/kb/face/0.png";
  layerLeft.value = "/pet/kb/lefthand/leftup.png";
  layerRight.value = "/pet/kb/righthand/rightup.png";
  leftPrefix = "/pet/kb/lefthand/";
  rightPrefix = "/pet/kb/righthand/";
  leftUpName = "leftup";
  rightUpName = "rightup";
  leftMatrix = [
    [17],
    [16],
    [82],
  ];
  rightMatrix = [
    [40],
    [37],
    [39],
    [38],
  ];
  fetch("/pet/kb/config.json")
    .then((r) => r.json())
    .then((cfg) => {
      leftMatrix = cfg?.keyboard?.lefthand ?? leftMatrix;
      rightMatrix = cfg?.keyboard?.righthand ?? rightMatrix;
    })
    .catch(() => {
      /* 用默认矩阵 */
    });
}

async function buildModel(id: string, mode: PetSettingsView["mode"]) {
  loadErr.value = "";
  if (id === "builtin") {
    buildBuiltin();
    return;
  }
  try {
    const files = await invoke<AssetFile[]>("pet_model_assets", { id, mode });
    revokeAssets();
    for (const f of files) assetUrls.set(f.rel, toBlobUrl(f));
    const url = (rel: string) => assetUrls.get(rel) ?? null;
    const cfg = await invoke<Record<string, unknown>>("pet_model_config", { id });

    if (mode === "standard") {
      layerBg.value = url("img/standard/mousebg.png");
      layerCat.value = url("img/standard/cat.png");
      layerFace.value = url("img/standard/face/0.png");
      layerLeft.value = url("img/standard/hand/0.png");
      layerRight.value = null;
      leftPrefix = "img/standard/hand/";
      rightPrefix = null;
      leftUpName = "0";
      rightUpName = "0";
      leftMatrix = (cfg as any)?.standard?.hand ?? [];
      rightMatrix = [];
    } else {
      const m = mode === "gamepad" ? "gamepad" : "keyboard";
      layerBg.value = url(`img/${m}/bg.png`);
      layerCat.value = url(`img/${m}/cat.png`);
      layerFace.value = url(`img/${m}/face/0.png`);
      layerLeft.value = url(`img/${m}/lefthand/leftup.png`) ?? url(`img/${m}/lefthand/0.png`);
      layerRight.value = url(`img/${m}/righthand/rightup.png`) ?? url(`img/${m}/righthand/0.png`);
      leftPrefix = `img/${m}/lefthand/`;
      rightPrefix = `img/${m}/righthand/`;
      leftUpName = assetUrls.has(`img/${m}/lefthand/leftup.png`) ? "leftup" : "0";
      rightUpName = assetUrls.has(`img/${m}/righthand/rightup.png`) ? "rightup" : "0";
      leftMatrix = (cfg as any)?.[m]?.lefthand ?? (cfg as any)?.keyboard?.lefthand ?? [];
      rightMatrix = (cfg as any)?.[m]?.righthand ?? (cfg as any)?.keyboard?.righthand ?? [];
    }
    if (!layerCat.value && !layerLeft.value) {
      loadErr.value = "该模型没有可用的图片素材，已回退内置模型";
      buildBuiltin();
    }
  } catch (e) {
    loadErr.value = String(e).replace(/^.*Error: /, "");
    buildBuiltin();
  }
}

/** 换帧：只在 File 存在时才切换，避免出现破图 */
function setFrame(
  which: "left" | "right",
  frame: string
) {
  const prefix = which === "left" ? leftPrefix : rightPrefix;
  if (!prefix) return;
  const next = `${prefix}${frame}.png`;
  if (assetUrls.size > 0) {
    // 自定义模型：素材缺失就保持当前帧
    const url = assetUrls.get(next);
    if (!url) return;
    if (which === "left") layerLeft.value = url;
    else layerRight.value = url;
    return;
  }
  // 内置素材：静态路径，先探测存在性再换，避免破图
  const probe = new Image();
  probe.onload = () => {
    if (which === "left") layerLeft.value = next;
    else layerRight.value = next;
  };
  probe.src = next;
}

function pressLeft(frame: string) {
  setFrame("left", frame);
  if (revertTimer) clearTimeout(revertTimer);
  revertTimer = window.setTimeout(() => {
    setFrame("left", leftUpName);
    setFrame("right", rightUpName);
  }, 260);
}

function pressRight(frame: string) {
  setFrame("right", frame);
  if (revertTimer) clearTimeout(revertTimer);
  revertTimer = window.setTimeout(() => {
    setFrame("left", leftUpName);
    setFrame("right", rightUpName);
  }, 260);
}

const LEFT_ZONE = new Set([
  9, 16, 17, 18, 20, 27, 49, 50, 51, 52, 53, 65, 66, 67, 68, 69, 70, 71, 81, 83, 84,
  86, 87, 88, 90, 192, 219, 221,
]);
const RIGHT_ZONE = new Set([
  8, 13, 32, 35, 36, 37, 38, 39, 40, 45, 46, 54, 55, 56, 57, 58, 72, 73, 74, 75, 76,
  77, 78, 79, 80, 82, 85, 89, 186, 187, 188, 189, 190, 191, 220, 222,
]);

function onInput(p: { kind: string; vk: number }) {
  if (p.kind !== "key") return;
  const vk = p.vk;
  const li = leftMatrix.findIndex((row) => row.includes(vk));
  if (li >= 0) {
    pressLeft(String(li));
    return;
  }
  const ri = rightMatrix.findIndex((row) => row.includes(vk));
  if (ri >= 0) {
    pressRight(String(ri));
    return;
  }
  // 矩阵外按键的启发式：按左右手位抬爪
  if (LEFT_ZONE.has(vk)) {
    pressLeft("0");
  } else if (RIGHT_ZONE.has(vk)) {
    pressRight("0");
  } else {
    alternateSide = !alternateSide;
    if (alternateSide) pressLeft("0");
    else pressRight("0");
  }
}

async function startDrag() {
  try {
    await getCurrentWebviewWindow().startDragging();
  } catch {
    /* 忽略 */
  }
}

async function savePosition() {
  try {
    const w = getCurrentWebviewWindow();
    const pos = await w.outerPosition();
    const scale = await w.scaleFactor();
    await invoke("pet_save_position", {
      x: pos.x / scale,
      y: pos.y / scale,
    });
  } catch {
    /* 忽略 */
  }
}

function onMouseUp() {
  void savePosition();
}

onMounted(async () => {
  await loadSettings();
  refresh();
  pollTimer = window.setInterval(refresh, 5000);
  try {
    unlistenInput = await listen<{ kind: string; vk: number }>("pet-input", (e) =>
      onInput(e.payload)
    );
    unlistenSettings = await listen<PetSettingsView>("pet-settings-changed", () => {
      void loadSettings();
    });
  } catch {
    /* 浏览器直开时忽略 */
  }
  window.addEventListener("mouseup", onMouseUp);
});

onUnmounted(() => {
  unlistenInput?.();
  unlistenSettings?.();
  clearInterval(pollTimer);
  if (revertTimer) clearTimeout(revertTimer);
  revokeAssets();
  window.removeEventListener("mouseup", onMouseUp);
});
</script>

<template>
  <div class="pet" @mousedown="startDrag" @mouseup="onMouseUp">
    <div class="stage" :class="{ mirror: settings?.mirror }" :style="{ opacity: (settings?.opacity ?? 100) / 100 }">
      <img v-if="layerBg" class="layer" :src="layerBg" alt="" draggable="false" />
      <img v-if="layerCat" class="layer" :src="layerCat" alt="" draggable="false" />
      <img v-if="layerLeft" class="layer" :src="layerLeft" alt="" draggable="false" />
      <img v-if="layerRight" class="layer" :src="layerRight" alt="" draggable="false" />
      <img v-if="layerFace" class="layer" :src="layerFace" alt="" draggable="false" />
    </div>
    <div class="badge">
      <span>{{ fmtDuration(seconds) }}</span>
      <span class="sep">·</span>
      <span title="自启动以来键入次数">{{ keys }} 键</span>
      <span class="sep">·</span>
      <span title="自启动以来点击次数">{{ clicks }} 击</span>
      <span v-if="loadErr" class="sep" :title="loadErr">· 模型异常</span>
    </div>
  </div>
</template>

<style>
html[data-mode="pet"],
html[data-mode="pet"] body {
  margin: 0;
  background: transparent !important;
  overflow: hidden;
}
</style>

<style scoped>
.pet {
  height: 100vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-end;
  user-select: none;
  -webkit-app-region: drag;
}

.stage {
  position: relative;
  width: 372px;
  aspect-ratio: 612 / 354;
  transition: transform 0.15s;
}

.stage.mirror {
  transform: scaleX(-1);
}

.layer {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}

.badge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: -4px;
  font-size: 11px;
  color: #cfd5e2;
  background: rgba(15, 17, 23, 0.82);
  border: 1px solid #2a2f3d;
  border-radius: 999px;
  padding: 2px 10px;
  font-variant-numeric: tabular-nums;
}

.sep {
  color: #4b5563;
}
</style>
