<script setup lang="ts">
import { computed } from "vue";
import type { SegSlice } from "../types";
import { colorFor } from "../lib/colors";
import { fmtDuration } from "../lib/format";

const props = defineProps<{ segments: SegSlice[] }>();

const dayStart = computed(() => {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return Math.floor(d.getTime() / 1000);
});
const dayEnd = computed(() => dayStart.value + 86400);
const now = computed(() => Math.floor(Date.now() / 1000));
// 统一 24 小时比例尺：记录块与"当前"线同轴
const SPAN = 86400;

interface Block {
  key: string;
  name: string;
  color: string;
  left: number;
  width: number;
  tip: string;
}

const blocks = computed<Block[]>(() =>
  props.segments
    .filter((s) => s.endTs > dayStart.value)
    .map((s) => {
      const start = Math.max(s.startTs, dayStart.value);
      const end = Math.min(s.endTs, dayEnd.value);
      const name = s.appName.replace(/\.exe$/i, "");
      return {
        key: `${s.appName}-${s.startTs}`,
        name,
        color: colorFor(s.appName),
        left: ((start - dayStart.value) / SPAN) * 100,
        width: Math.max(0.15, ((end - start) / SPAN) * 100),
        tip: `${name} · ${fmtDuration(end - start)}`,
      };
    })
);

const ticks = [0, 3, 6, 9, 12, 15, 18, 21, 24];
const nowPct = computed(() => ((now.value - dayStart.value) / SPAN) * 100);
</script>

<template>
  <div class="timeline">
    <div class="track">
      <div
        v-for="b in blocks"
        :key="b.key"
        class="block"
        :style="{ left: b.left + '%', width: b.width + '%', background: b.color }"
        :title="b.tip"
      ></div>
      <div v-if="nowPct <= 100" class="now" :style="{ left: nowPct + '%' }"></div>
    </div>
    <div class="ruler">
      <span
        v-for="t in ticks"
        :key="t"
        class="tick"
        :style="{ left: (t / 24) * 100 + '%' }"
        >{{ t % 6 === 0 ? t + "时" : "" }}</span
      >
    </div>
  </div>
</template>

<style scoped>
.timeline {
  position: relative;
}

.track {
  position: relative;
  height: 30px;
  background: #10131b;
  border: 1px solid #232936;
  border-radius: 8px;
  overflow: hidden;
}

.block {
  position: absolute;
  top: 4px;
  bottom: 4px;
  border-radius: 3px;
  opacity: 0.92;
  cursor: default;
}

.block:hover {
  opacity: 1;
  outline: 1px solid rgba(255, 255, 255, 0.35);
}

.now {
  position: absolute;
  top: 2px;
  bottom: 2px;
  width: 2px;
  background: #f87171;
  border-radius: 1px;
}

.ruler {
  position: relative;
  height: 16px;
  margin-top: 2px;
}

.tick {
  position: absolute;
  font-size: 10px;
  color: #5c6474;
  transform: translateX(-50%);
  white-space: nowrap;
}

.tick:last-child {
  transform: translateX(-100%);
}
</style>
