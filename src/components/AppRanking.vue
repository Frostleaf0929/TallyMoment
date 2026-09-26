<script setup lang="ts">
import { computed } from "vue";
import { NProgress } from "naive-ui";
import type { AppUsage } from "../types";
import { colorFor } from "../lib/colors";
import { fmtDuration } from "../lib/format";

const props = defineProps<{ apps: AppUsage[] }>();

const max = computed(() => Math.max(1, ...props.apps.map((a) => a.seconds)));

const shown = computed(() =>
  props.apps.slice(0, 8).map((a) => ({
    ...a,
    name: a.name.replace(/\.exe$/i, ""),
    color: colorFor(a.name),
    pct: Math.round((a.seconds / max.value) * 100),
  }))
);
</script>

<template>
  <div class="ranking">
    <div v-for="a in shown" :key="a.name" class="row">
      <span class="dot" :style="{ background: a.color }"></span>
      <span class="name" :title="a.name">{{ a.displayName }}</span>
      <span class="time">{{ fmtDuration(a.seconds) }}</span>
      <div class="bar">
        <NProgress
          type="line"
          :percentage="a.pct"
          :show-indicator="false"
          :height="6"
          border-radius="3px"
          :color="a.color"
          rail-color="#1c212d"
        />
      </div>
    </div>
    <p v-if="!shown.length" class="empty">今天还没有记录</p>
  </div>
</template>

<style scoped>
.ranking {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.row {
  display: grid;
  grid-template-columns: 10px 1fr auto;
  grid-template-rows: auto auto;
  column-gap: 10px;
  align-items: center;
}

.dot {
  grid-row: 1;
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.name {
  font-size: 13px;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.time {
  font-size: 12px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.bar {
  grid-column: 2 / 4;
  margin-top: 4px;
}

.empty {
  color: var(--text-faint);
  font-size: 13px;
  text-align: center;
  padding: 16px 0;
}
</style>
