<script setup lang="ts">
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { DayReport } from "../types";
import { fmtDuration } from "../lib/format";
import HourlyChart from "./HourlyChart.vue";

/** 某一天的对照详情：完成率 / 使用时长 / 小时分布 / 日志与图片
 *  同时用于待办页的弹层与「详细 · 事项」页 */
const props = defineProps<{ date: string }>();

const report = ref<DayReport | null>(null);
const stats = ref<{ todayDone: number; weekRate: number; ontimeRate: number; avgMinutes: number } | null>(
  null
);
const note = ref("");
const images = ref<string[]>([]);
const thumbs = ref<string[]>([]);
const imgLoading = ref(false);
const preview = ref("");

/** 完成率：没有数据时后端给 -1，显示"—"而不是 -100% */
const pct = (v: number | undefined) => (v === undefined || v < 0 ? "—" : `${Math.round(v * 100)}%`);

/** 前端缩成 320px 缩略图：原图直接进 DOM 是卡顿主因 */
function toThumb(dataUrl: string): Promise<string> {
  return new Promise((resolve) => {
    const img = new Image();
    img.decoding = "async";
    img.onload = () => {
      try {
        const k = Math.min(1, 320 / Math.max(img.width, img.height));
        const cv = document.createElement("canvas");
        cv.width = Math.max(1, Math.round(img.width * k));
        cv.height = Math.max(1, Math.round(img.height * k));
        const ctx = cv.getContext("2d");
        if (!ctx) return resolve(dataUrl);
        ctx.drawImage(img, 0, 0, cv.width, cv.height);
        resolve(cv.toDataURL("image/jpeg", 0.82));
      } catch {
        resolve(dataUrl);
      }
    };
    img.onerror = () => resolve(dataUrl);
    img.src = dataUrl;
  });
}

async function load(date: string) {
  if (!date) return;
  report.value = null;
  note.value = "";
  images.value = [];
  thumbs.value = [];
  const [rep, n, st] = await Promise.all([
    invoke<DayReport>("day_report", { date }).catch(() => null),
    invoke<{ content: string; images: string[] } | null>("notes_get", { date }).catch(() => null),
    invoke<typeof stats.value>("todo_stats").catch(() => null),
    ]);
  report.value = rep;
  stats.value = st;
  note.value = (n?.content ?? "").split(String.fromCharCode(10)).slice(0, 4).join(" ");
  const names = n?.images ?? [];
  if (!names.length) return;
  imgLoading.value = true;
  try {
    const list = await invoke<[string, string, string][]>("notes_images", { date, names });
    const urls = list.map(([, mime, b64]) => `data:${mime};base64,${b64}`);
    images.value = urls;
    thumbs.value = await Promise.all(urls.map(toThumb));
  } catch {
    /* 忽略 */
  } finally {
    imgLoading.value = false;
  }
}

watch(() => props.date, load, { immediate: true });
</script>

<template>
  <div class="daydetail">
    <div class="dcards">
      <div class="dstat"><p class="dl">今日完成</p><p class="dv">{{ stats?.todayDone ?? 0 }} 件</p></div>
      <div class="dstat"><p class="dl">本周完成率</p><p class="dv">{{ pct(stats?.weekRate) }}</p></div>
      <div class="dstat"><p class="dl">按时完成率</p><p class="dv">{{ pct(stats?.ontimeRate) }}</p></div>
      <div class="dstat"><p class="dl">平均完成用时</p><p class="dv">{{ stats?.avgMinutes ?? 0 }} 分</p></div>
    </div>

    <div class="dcards">
      <div class="dstat"><p class="dl">使用时长</p><p class="dv">{{ fmtDuration(report?.totalSeconds ?? 0) }}</p></div>
      <div class="dstat"><p class="dl">应用数</p><p class="dv">{{ report?.appCount ?? 0 }} 个</p></div>
      <div class="dstat"><p class="dl">键 / 击</p><p class="dv">{{ report?.keys ?? 0 }} / {{ report?.clicks ?? 0 }}</p></div>
      <div class="dstat"><p class="dl">专注块</p><p class="dv">{{ report?.blockIndex ?? 0 }} 个</p></div>
    </div>

    <div class="dsec">
      <p class="dl">那天的小时分布</p>
      <HourlyChart :slices="report?.hourly ?? []" :height="180" />
    </div>

    <div class="dsec">
      <p class="dl">那天写了什么</p>
      <p class="dtext">{{ note || "（这天还没有日志，可在日志卡片里补写）" }}</p>
    </div>

    <div v-if="thumbs.length" class="dsec">
      <p class="dl">那天的图片（{{ thumbs.length }} 张）</p>
      <div class="dimgs">
        <img
          v-for="(src, i) in thumbs"
          :key="i"
          :src="src"
          alt=""
          loading="lazy"
          decoding="async"
          @click="preview = images[i]"
        />
      </div>
      <p v-if="imgLoading" class="dtext">图片加载中…</p>
    </div>

    <div class="dsec">
      <p class="dl">状态 / 分析 / 完成率</p>
      <p class="dtext">框架已就位，细化的状态判定、分析与完成率对比后续继续打磨。</p>
    </div>

    <Teleport to="body">
      <div v-if="preview" class="mask" @click.self="preview = ''">
        <img class="big" :src="preview" alt="" />
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.daydetail {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.dcards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 10px;
}

.dstat {
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-md);
  padding: 10px 12px;
}

.dl {
  margin: 0 0 4px;
  font-size: 11px;
  color: var(--text-muted);
}

.dv {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.dsec {
  border-top: 1px solid var(--border);
  padding-top: 10px;
}

.dtext {
  margin: 0;
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.7;
}

.dimgs {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.dimgs img {
  width: 108px;
  height: 80px;
  object-fit: cover;
  border-radius: var(--r-sm);
  border: 1px solid var(--border);
  cursor: zoom-in;
}

.mask {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.55);
}

.big {
  max-width: min(900px, 92%);
  max-height: 92%;
  border-radius: var(--r-md);
  box-shadow: var(--card-shadow-hover);
}
</style>
