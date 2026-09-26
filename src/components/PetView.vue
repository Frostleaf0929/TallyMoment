<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { fmtDuration } from "../lib/format";

// 兔子洞模型（Bongo Cat Mver 标准模式分层素材）：全画布 612x354 逐层叠加
const HAND_FRAMES = 14;
const layerCat = "/pet/cat.png";
const layerFace = "/pet/face/0.png";
const layerKeyboard = ref("/pet/keyboard/0.png");
const layerHand = ref("/pet/hand/0.png");

const seconds = ref(0);
const keys = ref(0);
const clicks = ref(0);
let unlisten: UnlistenFn | undefined;
let pressTimer: number | undefined;
let pollTimer: number | undefined;
let nextHand = 1;

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

function press(handN: number) {
  layerHand.value = `/pet/hand/${handN}.png`;
  layerKeyboard.value = `/pet/keyboard/${handN}.png`;
  if (pressTimer) clearTimeout(pressTimer);
  pressTimer = window.setTimeout(() => {
    layerHand.value = "/pet/hand/0.png";
    layerKeyboard.value = "/pet/keyboard/0.png";
  }, 260);
}

function onInput(kind: string) {
  if (kind === "key") {
    press(nextHand);
    nextHand = nextHand >= HAND_FRAMES ? 1 : nextHand + 1;
  } else if (kind === "click") {
    press(nextHand);
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
    layerCat,
    layerFace,
    ...Array.from({ length: HAND_FRAMES + 1 }, (_, i) => `/pet/hand/${i}.png`),
    ...Array.from({ length: HAND_FRAMES + 1 }, (_, i) => `/pet/keyboard/${i}.png`),
  ];
  files.forEach((src) => {
    const img = new Image();
    img.src = src;
  });
  refresh();
  pollTimer = window.setInterval(refresh, 5000);
  unlisten = await listen<string>("pet-input", (e) => onInput(e.payload));
});
onUnmounted(() => {
  unlisten?.();
  clearInterval(pollTimer);
  if (pressTimer) clearTimeout(pressTimer);
});
</script>

<template>
  <div class="pet" @mousedown="startDrag">
    <div class="stage">
      <img class="layer" :src="layerCat" alt="" draggable="false" />
      <img class="layer" :src="layerKeyboard" alt="" draggable="false" />
      <img class="layer" :src="layerHand" alt="" draggable="false" />
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
.pet-root,
.pet-root body {
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
  width: 360px;
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
