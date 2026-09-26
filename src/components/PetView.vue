<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { fmtDuration } from "../lib/format";

// 兔子洞 keyboard 模式（内置）或已导入的 Mver 模型：分层渲染 + 按键矩阵映射
interface PetSettingsView {
  scale: number;
  opacity: number;
  mirror: boolean;
  activeModel: string;
  activeModelDir: string | null;
  mode: "keyboard" | "standard";
}

const settings = ref<PetSettingsView | null>(null);
const seconds = ref(0);
const keys = ref(0);
const clicks = ref(0);

const layerBg = ref<string | null>(null);
const layerCat = ref<string | null>(null);
const layerFace = ref<string | null>(null);
const layerLeft = ref<string | null>(null);
const layerRight = ref<string | null>(null);

// 按键矩阵（行号 = 帧编号，行内 = VK 码）
let leftMatrix: number[][] = [];
let rightMatrix: number[][] = [];
// 抬爪帧的文件名（keyboard 模式为 leftup/rightup；standard 模式用 0 号帧）
let leftUpName = "leftup";
let rightUpName = "rightup";

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
    await buildModel(s.activeModel, s.activeModelDir, s.mode);
  } catch {
    /* 忽略 */
  }
}

async function buildModel(id: string, dir: string | null, mode: "keyboard" | "standard") {
  if (id === "builtin" || !dir) {
    layerBg.value = "/pet/kb/bg.png";
    layerCat.value = "/pet/kb/cat.png";
    layerFace.value = "/pet/kb/face/0.png";
    layerLeft.value = "/pet/kb/lefthand/leftup.png";
    layerRight.value = "/pet/kb/righthand/rightup.png";
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
    try {
      const cfg = await fetch("/pet/kb/config.json").then((r) => r.json());
      leftMatrix = cfg?.keyboard?.lefthand ?? leftMatrix;
      rightMatrix = cfg?.keyboard?.righthand ?? rightMatrix;
    } catch {
      /* 用默认矩阵 */
    }
    return;
  }

  // 自定义模型：经 asset 协议读取
  const f = (rel: string) => convertFileSrc(`${dir}${dir.endsWith("/") ? "" : "/"}${rel}`);
  try {
    const cfg = await invoke<Record<string, any>>("pet_model_config", { id });
    if (mode === "keyboard") {
      layerBg.value = f("img/keyboard/bg.png");
      layerCat.value = f("img/keyboard/cat.png");
      layerFace.value = f("img/keyboard/face/0.png");
      leftUpName = "leftup";
      rightUpName = "rightup";
      layerLeft.value = f("img/keyboard/lefthand/leftup.png");
      layerRight.value = f("img/keyboard/righthand/rightup.png");
      leftMatrix = cfg?.keyboard?.lefthand ?? [];
      rightMatrix = cfg?.keyboard?.righthand ?? [];
    } else {
      layerBg.value = null;
      layerCat.value = f("img/standard/cat.png");
      layerFace.value = f("img/standard/face/0.png");
      leftUpName = "0";
      rightUpName = "0";
      layerLeft.value = f("img/standard/hand/0.png");
      layerRight.value = null;
      leftMatrix = cfg?.standard?.hand ?? [];
      rightMatrix = [];
    }
  } catch {
    /* 回退内置 */
    layerBg.value = "/pet/kb/bg.png";
    layerCat.value = "/pet/kb/cat.png";
    layerFace.value = "/pet/kb/face/0.png";
  }
}

function pressLeft(frame: string) {
  layerLeft.value =
    layerLeft.value && layerLeft.value.includes("/")
      ? layerLeft.value.replace(/[^/]+\.png$/, `${frame}.png`)
      : layerLeft.value;
  if (revertTimer) clearTimeout(revertTimer);
  revertTimer = window.setTimeout(() => {
    layerLeft.value =
      layerLeft.value?.replace(/[^/]+\.png$/, `${leftUpName}.png`) ?? layerLeft.value;
    layerRight.value =
      layerRight.value?.replace(/[^/]+\.png$/, `${rightUpName}.png`) ?? layerRight.value;
  }, 260);
}

function pressRight(frame: string) {
  layerRight.value =
    layerRight.value && layerRight.value.includes("/")
      ? layerRight.value.replace(/[^/]+\.png$/, `${frame}.png`)
      : layerRight.value;
  if (revertTimer) clearTimeout(revertTimer);
  revertTimer = window.setTimeout(() => {
    layerLeft.value =
      layerLeft.value?.replace(/[^/]+\.png$/, `${leftUpName}.png`) ?? layerLeft.value;
    layerRight.value =
      layerRight.value?.replace(/[^/]+\.png$/, `${rightUpName}.png`) ?? layerRight.value;
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
  // 预加载内置帧
  [
    "/pet/kb/bg.png",
    "/pet/kb/cat.png",
    "/pet/kb/face/0.png",
    "/pet/kb/lefthand/leftup.png",
    "/pet/kb/righthand/rightup.png",
  ].forEach((src) => {
    const img = new Image();
    img.src = src;
  });
  await loadSettings();
  refresh();
  pollTimer = window.setInterval(refresh, 5000);
  unlistenInput = await listen<{ kind: string; vk: number }>("pet-input", (e) =>
    onInput(e.payload)
  );
  unlistenSettings = await listen<PetSettingsView>("pet-settings-changed", () => {
    void loadSettings();
  });
  window.addEventListener("mouseup", onMouseUp);
});

onUnmounted(() => {
  unlistenInput?.();
  unlistenSettings?.();
  clearInterval(pollTimer);
  if (revertTimer) clearTimeout(revertTimer);
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
