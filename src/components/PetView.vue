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

interface Live2dBundle {
  model3: Record<string, unknown>;
  model3Rel: string;
  files: AssetFile[];
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

/* ---------- Live2D（Cubism Core for Web + pixi-live2d-display） ---------- */
const canvasEl = ref<HTMLCanvasElement | null>(null);
const live2dOn = ref(false);
const live2dMsg = ref("");
interface PixiAppLike {
  destroy: (a?: boolean, b?: unknown) => void;
  ticker: { maxFPS: number };
  stage: { addChild: (c: unknown) => void };
}
let pixiApp: PixiAppLike | null = null;
let l2dModel: {
  destroy: () => void;
  width: number;
  height: number;
  scale: { set: (v: number) => void };
  anchor: { set: (x: number, y: number) => void };
  position: { set: (x: number, y: number) => void };
  expression: (i?: number) => void;
  internalModel?: { settings?: { expressions?: unknown[] } };
} | null = null;
let lastReact = 0;

/** 经典 script 引入 Cubism Core（/public 下的文件不能被 import） */
async function ensureCubismCore(): Promise<void> {
  const w = window as unknown as { Live2DCubismCore?: unknown };
  if (w.Live2DCubismCore) return;
  await new Promise<void>((resolve, reject) => {
    const s = document.createElement("script");
    s.src = "/live2d/live2dcubismcore.min.js";
    s.onload = () => resolve();
    s.onerror = () => reject(new Error("Live2D 运行时（Cubism Core）加载失败"));
    document.head.appendChild(s);
  });
}

function destroyLive2d() {
  try {
    l2dModel?.destroy();
  } catch {
    /* 忽略 */
  }
  try {
    pixiApp?.destroy(true, { children: true });
  } catch {
    /* 忽略 */
  }
  l2dModel = null;
  pixiApp = null;
  live2dOn.value = false;
}

/** 把 model3.json 里的相对引用全部换成 Blob URL，再交给 pixi 渲染 */
async function mountLive2d(bundle: Live2dBundle) {
  destroyLive2d();
  live2dMsg.value = "";
  await ensureCubismCore();
  const PIXI = (await import("pixi.js")) as unknown as {
    Application: new (o: Record<string, unknown>) => PixiAppLike;
  };
  const { Live2DModel } = (await import("pixi-live2d-display/cubism4")) as unknown as {
    Live2DModel: { from: (s: unknown, o?: unknown) => Promise<typeof l2dModel> };
  };

  const map = new Map<string, string>();
  for (const f of bundle.files) map.set(f.rel, toBlobUrl(f));
  const dir = bundle.model3Rel.replace(/[^/]+$/, "");
  const resolve = (rel: string) => map.get(dir + rel) ?? map.get(rel) ?? rel;

  const settings = JSON.parse(JSON.stringify(bundle.model3)) as Record<string, unknown> & {
    url?: string;
    FileReferences?: Record<string, unknown>;
  };
  settings.url = map.get(bundle.model3Rel) ?? "";
  const fr = (settings.FileReferences ?? {}) as Record<string, unknown>;
  if (typeof fr.Moc === "string") fr.Moc = resolve(fr.Moc);
  if (Array.isArray(fr.Textures)) fr.Textures = (fr.Textures as string[]).map(resolve);
  for (const k of ["Physics", "Pose", "DisplayInfo", "UserData"]) {
    if (typeof fr[k] === "string") fr[k] = resolve(fr[k] as string);
  }
  for (const e of (fr.Expressions ?? []) as { File?: string }[]) {
    if (e?.File) e.File = resolve(e.File);
  }
  for (const group of Object.values((fr.Motions ?? {}) as Record<string, { File?: string }[]>)) {
    for (const m of group ?? []) if (m?.File) m.File = resolve(m.File);
  }

  const canvas = canvasEl.value;
  if (!canvas) throw new Error("画布未就绪");
  const app = new PIXI.Application({
    view: canvas,
    width: BASE_W,
    height: BASE_H,
    backgroundAlpha: 0,
    antialias: true,
    autoStart: true,
    resolution: Math.min(window.devicePixelRatio || 1, 2),
  });
  if (!app) throw new Error("渲染器创建失败");
  pixiApp = app;
  app.ticker.maxFPS = 30;

  const model = await Live2DModel.from(settings, { autoInteract: false });
  if (!model) throw new Error("Live2D 模型加载失败");
  l2dModel = model;
  const k = Math.min(BASE_W / model.width, BASE_H / model.height) * 1.1;
  model.scale.set(k);
  model.anchor.set(0.5, 0.5);
  model.position.set(BASE_W / 2, BASE_H / 2 + 10);
  (app.stage as { addChild: (c: unknown) => void }).addChild(model);  live2dOn.value = true;
}

/** 按键时让 Live2D 模型换个表情（有冷却，避免连发） */
function reactLive2d() {
  const m = l2dModel;
  if (!m) return;
  const now = Date.now();
  if (now - lastReact < 200) return;
  lastReact = now;
  try {
    const list = m.internalModel?.settings?.expressions ?? [];
    if (list.length) m.expression(Math.floor(Math.random() * list.length));
  } catch {
    /* 忽略 */
  }
}

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
    destroyLive2d();
    buildBuiltin();
    return;
  }
  // 先试 Live2D：模型带 cat_model/*.model3.json 就用它渲染
  try {
    const bundle = await invoke<Live2dBundle>("pet_model_live2d", { id, mode });
    if (bundle && bundle.files?.length) {
      await mountLive2d(bundle);
      return;
    }
  } catch (e) {
    live2dMsg.value = String(e).replace(/^.*Error: /, "");
    live2dMsg.value = "";
  }
  destroyLive2d();

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
  // Live2D 模式：没有分层素材帧，改用表情反馈
  if (live2dOn.value) {
    reactLive2d();
    return;
  }
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

/** 真缩放：窗口大小 = 设计尺寸 × k，整层用 transform 缩放，
 *  而不是像以前那样固定 372px 布局、窗口一小就把桌宠裁掉一半 */
const k = ref(1);

function updateScale() {
  const w = window.innerWidth || BASE_W;
  k.value = Math.max(0.2, w / BASE_W);
}
const BASE_W = 372;
const BASE_H = 226;

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
  updateScale();
  window.addEventListener("resize", updateScale);
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
  window.removeEventListener("resize", updateScale);
  revokeAssets();
  window.removeEventListener("mouseup", onMouseUp);
});
</script>

<template>
  <div class="pet" @mousedown="startDrag" @mouseup="onMouseUp">
    <div class="scaler" :style="{ transform: `scale(${k})`, opacity: (settings?.opacity ?? 100) / 100 }">
      <canvas v-show="live2dOn" ref="canvasEl" class="l2d" :width="372" :height="226"></canvas>
      <div v-show="!live2dOn" class="stage" :class="{ mirror: settings?.mirror }">
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
        <span v-if="live2dOn" class="sep" title="正在用 Live2D 渲染">· L2D</span>
        <span v-if="loadErr" class="sep" :title="loadErr">· 模型异常</span>
      </div>
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
  width: 100vw;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  overflow: hidden;
  user-select: none;
}

/* 固定设计尺寸 372×226，整体 transform 缩放 → 真缩放而非裁剪 */
.scaler {
  position: relative;
  width: 372px;
  height: 226px;
  transform-origin: bottom center;
  flex: none;
}

.stage {
  position: absolute;
  left: 0;
  top: 0;
  width: 372px;
  aspect-ratio: 612 / 354;
  transition: transform 0.15s;
}

.l2d {
  position: absolute;
  left: 0;
  top: 0;
  width: 372px;
  height: 226px;
  pointer-events: none;
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
  position: absolute;
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: #cfd5e2;
  background: rgba(15, 17, 23, 0.82);
  border: 1px solid #2a2f3d;
  border-radius: 999px;
  padding: 2px 10px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.sep {
  color: #4b5563;
}
</style>
