<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { bgImgStyle, bgScrimOpacity } from "../lib/bgStyle";

/** 全屏休息提醒（提醒方式 = 全屏时由 reminder_full 窗口渲染）
 *  设计：分层柔光 + 呼吸动效 + 大号提示语 + 两个动作按钮 */
interface Payload {
  id: string;
  kind: string;
  refId: number;
  title: string;
  body: string;
  accent: string | null;
  actions: { id: string; label: string }[];
}

const item = ref<Payload | null>(null);
/** 自定义背景图（个性化里设置；没有则用默认分层柔光）。
 *  ⚠️ 全屏窗口是复用的：每次 reminder-show 都要重新挑图（顺序/随机轮换才生效） */
const bg = ref<{ url: string; fit: string; align: string; zoom: number; scrim: number } | null>(null);
const leaving = ref(false);
let unlisten: UnlistenFn | undefined;
let autoClose: number | undefined;

/** 渲染样式：与编辑器同一函数（唯一口径） */
const bgStyle = computed(() => (bg.value ? bgImgStyle(bg.value) : {}));
const scrimStyle = computed(() => ({
  opacity: bg.value ? bgScrimOpacity(bg.value.scrim) : "1",
}));

async function refreshBg() {
  try {
    const w = await invoke<{
      mime: string;
      data: string;
      fit: string;
      align: string;
      zoom: number;
      scrim: number;
    } | null>("reminder_bg_pick");
    bg.value = w ? { url: `data:${w.mime};base64,${w.data}`, fit: w.fit, align: w.align, zoom: w.zoom, scrim: w.scrim } : null;
  } catch {
    bg.value = null;
  }
}

function apply(p: Payload) {
  item.value = p;
  void refreshBg();
  if (autoClose) clearTimeout(autoClose);
  // 全屏页给足停留时间，但不无限挂着
  autoClose = window.setTimeout(() => void dismiss("ack"), 60_000);
}

onMounted(async () => {
  void refreshBg();
  try {
    const pending = await invoke<Payload[]>("reminder_pending");
    if (pending.length) apply(pending[pending.length - 1]);
    unlisten = await listen<Payload>("reminder-show", (e) => apply(e.payload));
  } catch {
    /* 忽略 */
  }
  // 前端就绪：请 Rust 侧显示窗口（创建时隐藏，防睡眠唤醒后首帧未渲染就露黑屏）
  void invoke("reminder_show_full_window").catch(() => {});
});

onUnmounted(() => {
  unlisten?.();
  if (autoClose) clearTimeout(autoClose);
});

/** 关掉这张全屏提醒：先回传动作（任务完成/稍后等），再收起窗口 */
async function dismiss(action: string) {
  const p = item.value;
  leaving.value = true;
  if (p) {
    try {
      await invoke("reminder_action", { kind: p.kind, refId: p.refId, action });
    } catch {
      /* 忽略 */
    }
  }
  window.setTimeout(async () => {
    try {
      await invoke("close_reminder_full");
    } catch {
      /* 忽略 */
    }
    item.value = null;
    leaving.value = false;
  }, 220);
}
</script>

<template>
  <div class="full" :class="{ leaving, hasbg: !!bg }">
    <div v-if="bg" class="bgbox">
      <img class="bgimg" :src="bg.url" alt="" :style="bgStyle" />
    </div>
    <div v-if="bg" class="scrim" :style="scrimStyle"></div>
    <div class="glow g1"></div>
    <div class="glow g2"></div>
    <div class="glow g3"></div>

    <div class="center">
      <div class="ring">
        <div class="ring-inner"></div>
      </div>
      <p class="mark">拾刻 · 休息提醒</p>
      <h1>{{ item?.title || "该休息了" }}</h1>
      <p class="body">{{ item?.body || "站起来走走、看看远处，给眼睛一点时间。" }}</p>
      <div class="acts">
        <button class="primary" @click="dismiss('ack')">知道了</button>
        <button v-for="a in (item?.actions ?? []).filter((x) => x.id !== 'ack')" :key="a.id" class="ghost" @click="dismiss(a.id)">
          {{ a.label }}
        </button>
        <button class="ghost" @click="dismiss('snooze5')">再等 5 分钟</button>
      </div>
      <p class="tip">按 Esc 也可以关闭</p>
    </div>
  </div>
</template>

<style scoped>
.full {
  position: fixed;
  inset: 0;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background:
    radial-gradient(60% 50% at 20% 10%, rgba(123, 132, 236, 0.22), transparent 70%),
    radial-gradient(55% 45% at 85% 20%, rgba(88, 179, 196, 0.18), transparent 72%),
    radial-gradient(70% 60% at 55% 105%, rgba(160, 143, 224, 0.2), transparent 75%),
    #0d1016;
  color: #e6e9f2;
  font-family: "Segoe UI Variable", "Segoe UI", "Microsoft YaHei UI", system-ui, sans-serif;
  transition: opacity 0.22s ease, transform 0.22s ease;
  animation: fadein 0.4s ease both;
}

.full.leaving {
  opacity: 0;
  transform: scale(1.02);
}

@keyframes fadein {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

/* 自定义背景图 + 暗色压层（保证文字可读） */
/* 背景裁切容器 + 铺满图片。
   ⚠️ 关键修复（2026-10-05）：<img> 必须有显式 width/height 才会铺满容器；
   只写 inset:0 时 img 会按"原始像素尺寸"渲染（replaced element 的尺寸规则），
   结果原图比窗口大→只看到左上角（观感=位置偏移/缩放不对）、比窗口小→右下露底色（黑区）。
   编辑器预览一直有宽高所以正常，实机没有 → 这就是"预览与实机不一致"的根源。 */
.bgbox {
  position: absolute;
  inset: 0;
  overflow: hidden;
}
.bgimg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  display: block;
}

.scrim {
  position: absolute;
  inset: 0;
  background: linear-gradient(180deg, rgba(8, 10, 16, 0.62), rgba(8, 10, 16, 0.78));
}

.full.hasbg .glow {
  display: none;
}

.glow {
  position: absolute;
  border-radius: 50%;
  filter: blur(90px);
  opacity: 0.5;
  animation: float 14s ease-in-out infinite;
}

.g1 {
  width: 460px;
  height: 460px;
  left: 6%;
  top: 8%;
  background: #6a74e0;
}

.g2 {
  width: 380px;
  height: 380px;
  right: 8%;
  top: 18%;
  background: #4ea9b8;
  animation-delay: -4s;
}

.g3 {
  width: 520px;
  height: 520px;
  left: 38%;
  bottom: -12%;
  background: #8f7fd8;
  animation-delay: -8s;
}

@keyframes float {
  0%,
  100% {
    transform: translateY(0) scale(1);
  }
  50% {
    transform: translateY(-24px) scale(1.05);
  }
}

.center {
  position: relative;
  z-index: 1;
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 40px;
  max-width: 760px;
}

.ring {
  width: 128px;
  height: 128px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  background: conic-gradient(from 0deg, rgba(123, 132, 236, 0.9), rgba(88, 179, 196, 0.9), rgba(160, 143, 224, 0.9), rgba(123, 132, 236, 0.9));
  animation: spin 9s linear infinite;
  box-shadow: 0 0 60px rgba(123, 132, 236, 0.35);
}

.ring-inner {
  width: 104px;
  height: 104px;
  border-radius: 50%;
  background: #0d1016;
  display: grid;
  place-items: center;
  animation: spin 9s linear infinite reverse;
}

.ring-inner::after {
  content: "";
  width: 34px;
  height: 34px;
  border-radius: 50%;
  border: 3px solid rgba(230, 233, 242, 0.85);
  border-top-color: transparent;
  animation: spin 2.6s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.mark {
  margin: 10px 0 0;
  font-size: 12px;
  letter-spacing: 0.28em;
  color: rgba(230, 233, 242, 0.5);
}

h1 {
  margin: 0;
  font-size: 46px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.body {
  margin: 0;
  font-size: 16px;
  line-height: 1.8;
  color: rgba(230, 233, 242, 0.72);
  max-width: 560px;
}

.acts {
  margin-top: 18px;
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  justify-content: center;
}

.primary,
.ghost {
  border-radius: 999px;
  font-family: inherit;
  font-size: 14px;
  padding: 10px 26px;
  cursor: pointer;
  transition: transform 0.16s ease, background 0.16s ease, color 0.16s ease;
}

.primary {
  border: 0;
  background: linear-gradient(135deg, #7b84ec, #58b3c4);
  color: #fff;
  font-weight: 600;
}

.primary:hover {
  transform: translateY(-1px) scale(1.02);
}

.ghost {
  border: 1px solid rgba(230, 233, 242, 0.22);
  background: transparent;
  color: rgba(230, 233, 242, 0.85);
}

.ghost:hover {
  background: rgba(230, 233, 242, 0.1);
}

.tip {
  margin: 14px 0 0;
  font-size: 11.5px;
  color: rgba(230, 233, 242, 0.35);
}
</style>
