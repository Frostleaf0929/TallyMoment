<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { fmtDuration } from "../lib/format";

// 兔子洞 keyboard 模式（Bongo Cat Mver 分层素材）：
// bg 键盘底图 + cat 本体 + 左右爪（按键映射来自 config.json 矩阵）+ 表情
const layerBg = "/pet/kb/bg.png";
const layerCat = "/pet/kb/cat.png";
const layerFace = "/pet/kb/face/0.png";
const layerLeft = ref("/pet/kb/lefthand/leftup.png");
const layerRight = ref("/pet/kb/righthand/rightup.png");

// 模型自带的按键矩阵（行号 = 素材编号，行内 = VK 码）
let leftMatrix: number[][] = [];
let rightMatrix: number[][] = [];

const seconds = ref(0);
const keys = ref(0);
const clicks = ref(0);
let unlisten: UnlistenFn | undefined;
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

const LEFT_ZONE = new Set([
  9, 16, 17, 18, 20, 27, 49, 50, 51, 52, 53, 65, 66, 67, 68, 69, 70, 71, 81,
  83, 84, 86, 87, 88, 90, 90, 192, 219, 221,
]);
const RIGHT_ZONE = new Set([
  8, 13, 32, 35, 36, 37, 38, 39, 40, 45, 46, 54, 55, 56, 57, 58, 72, 73, 74,
  75, 76, 77, 78, 79, 80, 82, 85, 89, 186, 187, 188, 189, 190, 191, 220, 222,
]);

function pressLeft(idx: number) {
  layerLeft.value = `/pet/kb/lefthand/${idx}.png`;
  if (revertTimer) clearTimeout(revertTimer);
  revertTimer = window.setTimeout(() => {
    layerLeft.value = "/pet/kb/lefthand/leftup.png";
    layerRight.value = "/pet/kb/righthand/rightup.png";
  }, 260);
}

function pressRight(idx: number) {
  layerRight.value = `/pet/kb/righthand/${idx}.png`;
  if (revertTimer) clearTimeout(revertTimer);
  revertTimer = window.setTimeout(() => {
    layerLeft.value = "/pet/kb/lefthand/leftup.png";
    layerRight.value = "/pet/kb/righthand/rightup.png";
  }, 260);
}

function onInput(p: { kind: string; vk: number }) {
  if (p.kind !== "key") return;
  const vk = p.vk;
  // 模型矩阵优先：命中哪一行就用哪一帧（最真实）
  const li = leftMatrix.findIndex((row) => row.includes(vk));
  const ri = rightMatrix.findIndex((row) => row.includes(vk));
  if (li >= 0) {
    pressLeft(li);
    return;
  }
  if (ri >= 0) {
    pressRight(ri);
    return;
  }
  // 矩阵外按键的启发式：按左右手位分区抬爪，保证日常打字也有反馈
  if (LEFT_ZONE.has(vk)) {
    pressLeft(0);
  } else if (RIGHT_ZONE.has(vk)) {
    pressRight(0);
  } else {
    alternateSide = !alternateSide;
    if (alternateSide) pressLeft(0);
    else pressRight(0);
  }
}

async function startDrag() {
  try {
    await getCurrentWebviewWindow().startDragging();
  } catch {
    /* 忽略 */
  }
}

onMounted(async () => {
  // 预加载全部帧，避免按键时闪白
  const files = [
    layerBg,
    layerCat,
    layerFace,
    "/pet/kb/lefthand/0.png",
    "/pet/kb/lefthand/1.png",
    "/pet/kb/lefthand/2.png",
    "/pet/kb/lefthand/leftup.png",
    "/pet/kb/righthand/0.png",
    "/pet/kb/righthand/1.png",
    "/pet/kb/righthand/2.png",
    "/pet/kb/righthand/3.png",
    "/pet/kb/righthand/rightup.png",
  ];
  files.forEach((src) => {
    const img = new Image();
    img.src = src;
  });
  // 读取模型自带的按键映射
  try {
    const cfg = await fetch("/pet/kb/config.json").then((r) => r.json());
    leftMatrix = cfg?.keyboard?.lefthand ?? [];
    rightMatrix = cfg?.keyboard?.righthand ?? [];
  } catch {
    leftMatrix = [];
    rightMatrix = [];
  }
  refresh();
  pollTimer = window.setInterval(refresh, 5000);
  unlisten = await listen<{ kind: string; vk: number }>("pet-input", (e) =>
    onInput(e.payload)
  );
});
onUnmounted(() => {
  unlisten?.();
  clearInterval(pollTimer);
  if (revertTimer) clearTimeout(revertTimer);
});
</script>

<template>
  <div class="pet" @mousedown="startDrag">
    <div class="stage">
      <img class="layer" :src="layerBg" alt="" draggable="false" />
      <img class="layer" :src="layerCat" alt="" draggable="false" />
      <img class="layer" :src="layerLeft" alt="" draggable="false" />
      <img class="layer" :src="layerRight" alt="" draggable="false" />
      <img class="layer" :src="layerFace" alt="" draggable="false" />
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
