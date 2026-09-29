<script setup lang="ts">
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { goTab, notesIntent } from "../lib/uiState";
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

/** 完成率：后端已经按百分数给值（0~100），-1 = 没有数据显示"—"（此前前端又乘了一次 100，出现 1000%） */
const pct = (v: number | undefined) => (v === undefined || v < 0 ? "—" : `${Math.round(v)}%`);

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
  note.value = (n?.content ?? "").split(String.fromCharCode(10)).slice(0, 14).join(" ");
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

/** 去〈待办 · 日志〉编辑这天的日志（参照"返回待办"的胶囊按钮样式） */
function editNote() {
  notesIntent.value = { date: props.date, at: Date.now() };
  goTab("todo");
}
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

    <div class="dsec notecard">
      <div class="notehead">
        <p class="dl">那天写了什么{{ thumbs.length ? ` · ${thumbs.length} 张图片` : "" }}</p>
        <button class="editnote" title="到〈待办 · 日志〉编辑这天" @click="editNote">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 20h4L19 9l-4-4L4 16v4zM13 6l4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>编辑</span>
        </button>
      </div>
      <div class="notebody">
        <p class="dtext notetext">{{ note || "（这天还没有日志，点右上角「编辑」去写）" }}</p>
        <div v-if="thumbs.length" class="dimgs">
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

.notecard {
  min-height: 150px;
}

.notehead {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

/* 编辑胶囊：与「返回待办」同款（圆形图标，悬停展开文字） */
.editnote {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: var(--r-full);
  background: var(--surface);
  color: var(--text-muted);
  cursor: pointer;
  overflow: hidden;
  transition: width var(--dur) cubic-bezier(0.22, 0.61, 0.36, 1), background var(--dur), color var(--dur), border-color var(--dur);
}

.editnote svg {
  width: 14px;
  height: 14px;
  flex: none;
}

.editnote span {
  max-width: 0;
  overflow: hidden;
  font-size: 11.5px;
  white-space: nowrap;
  opacity: 0;
  transition: max-width var(--dur) ease, opacity var(--dur) ease, margin-left var(--dur) ease;
}

.editnote:hover {
  width: 76px;
  color: var(--accent-text);
  border-color: var(--accent-border);
  background: var(--accent-soft);
}

.editnote:hover span {
  max-width: 44px;
  opacity: 1;
  margin-left: 5px;
}

.notebody {
  display: flex;
  gap: 14px;
  align-items: flex-start;
}

/* 文字在左占大头，图片在右固定宽 */
.notetext {
  flex: 1;
  min-width: 0;
  font-size: 12.5px;
}

.notebody .dimgs {
  flex: none;
  width: 190px;
  flex-direction: column;
}

.notebody .dimgs img {
  width: 100%;
  height: auto;
  max-height: 150px;
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
