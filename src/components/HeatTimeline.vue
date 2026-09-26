<script setup lang="ts">
import { computed } from "vue";
import type { SegSlice } from "../types";
import { colorFor } from "../lib/colors";

// 分钟级热力时间线：3 分钟一格，格色 = 主导应用色，透明度 = 活跃密度
const props = defineProps<{ segments: SegSlice[] }>();

const BUCKET = 180; // 秒/格
const CELLS = 480; // 24h

const dayStart = computed(() => {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return Math.floor(d.getTime() / 1000);
});
const dayEnd = computed(() => dayStart.value + 86400);
const now = computed(() => Math.floor(Date.now() / 1000));
const viewEnd = computed(() => Math.min(now.value, dayEnd.value));

interface Cell {
  i: number;
  fill: string;
  opacity: number;
  tip: string;
}

const cells = computed<Cell[]>(() => {
  const out: Cell[] = [];
  const endTs = viewEnd.value;
  for (let i = 0; i < CELLS; i++) {
    const cs = dayStart.value + i * BUCKET;
    const ce = Math.min(cs + BUCKET, endTs);
    if (ce <= cs) break;
    // 找出该格内重叠秒数最多的应用
    let best: { app: string; secs: number } | null = null;
    let total = 0;
    for (const s of props.segments) {
      const os = Math.max(s.startTs, cs);
      const oe = Math.min(s.endTs, ce);
      if (oe <= os) continue;
      const ov = oe - os;
      total += ov;
      if (!best || ov > best.secs) best = { app: s.appName, secs: ov };
    }
    if (!best || total <= 0) continue;
    const name = best.app.replace(/\.exe$/i, "");
    out.push({
      i,
      fill: colorFor(best.app),
      opacity: 0.3 + 0.7 * Math.min(1, total / BUCKET),
      tip: `${String(Math.floor((i * BUCKET) / 3600)).padStart(2, "0")}:${String(
        Math.floor(((i * BUCKET) % 3600) / 60)
      ).padStart(2, "0")} · ${name} · ${Math.round(total / 60)} 分钟`,
    });
  }
  return out;
});

const ticks = [0, 4, 8, 12, 16, 20, 24];
const nowPct = computed(() => ((now.value - dayStart.value) / 86400) * 100);
</script>

<template>
  <div class="heat">
    <svg class="strip" viewBox="0 0 480 34" preserveAspectRatio="none">
      <rect
        v-for="c in cells"
        :key="c.i"
        :x="c.i"
        y="2"
        width="1.02"
        height="30"
        rx="0.6"
        :fill="c.fill"
        :fill-opacity="c.opacity"
      >
        <title>{{ c.tip }}</title>
      </rect>
      <line
        v-if="nowPct <= 100"
        :x1="(nowPct / 100) * 480"
        x2="(nowPct / 100) * 480"
        y1="0"
        y2="34"
        stroke="#8a91a0"
        stroke-width="1"
        stroke-dasharray="2 2"
      />
    </svg>
    <div class="ruler">
      <span v-for="t in ticks" :key="t" class="tick" :style="{ left: (t / 24) * 100 + '%' }">
        {{ t }}时
      </span>
    </div>
  </div>
</template>

<style scoped>
.heat {
  position: relative;
}

.strip {
  width: 100%;
  height: 46px;
  display: block;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid var(--border);
  border-radius: var(--r-md);
}

.ruler {
  position: relative;
  height: 16px;
  margin-top: 3px;
}

.tick {
  position: absolute;
  font-size: 10px;
  color: var(--text-faint);
  transform: translateX(-50%);
  white-space: nowrap;
}

.tick:first-child {
  transform: none;
}

.tick:last-child {
  transform: translateX(-100%);
}
</style>
