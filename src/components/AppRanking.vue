<script setup lang="ts">
import { computed, watchEffect } from "vue";
import { NProgress } from "naive-ui";
import type { AppUsage } from "../types";
import { colorFor } from "../lib/colors";
import { fmtDuration } from "../lib/format";
import { chartColors } from "../lib/chartColors";
import { appsTopN, isLight } from "../lib/uiState";
import { appColorMode, appNameEnglish } from "../lib/appearance";
import { iconColor, iconUrl, requestIcons } from "../lib/appIcons";

const emit = defineEmits<{ (e: "pick", name: string): void }>();

const props = withDefaults(defineProps<{ apps: AppUsage[]; limit?: number }>(), { limit: 0 });

const max = computed(() => Math.max(1, ...props.apps.map((a) => a.seconds)));

const shown = computed(() => {
  const n = props.limit || appsTopN.value;
  return props.apps.slice(0, n).map((a, i) => ({
    ...a,
    rank: i + 1,
    // key = 数据库里的进程标识（跳转必须用它）；label = 展示名
    key: a.name,
    label: appNameEnglish.value ? a.name.replace(/\.exe$/i, "") : a.displayName,
    color:
      appColorMode.value === "accent"
        ? getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() || "#7b84ec"
        : appColorMode.value === "iconColor"
        ? iconColor(a.name) || colorFor(a.name)
        : colorFor(a.name),
    icon: appColorMode.value === "icon" ? iconUrl(a.name) : "",
    pct: Math.round((a.seconds / max.value) * 100),
  }));
});

/** 轨道色必须跟着主题走（此前写死深色，浅色模式下是一道黑线） */
const rail = computed(() => {
  void isLight.value;
  return chartColors().rail;
});

const rest = computed(() => Math.max(0, props.apps.length - shown.value.length));

// 需要图标时批量取一次
watchEffect(() => {
  if (appColorMode.value === "icon" || appColorMode.value === "iconColor") {
    requestIcons(shown.value.map((a) => a.key));
  }
});
</script>

<template>
  <div class="ranking">
    <button v-for="a in shown" :key="a.key" class="row" :title="`查看 ${a.label} 的明细`" @click="emit('pick', a.key)">
      <span class="rank">{{ a.rank }}</span>
      <img v-if="a.icon" class="iapp" :src="a.icon" alt="" draggable="false" />
      <span v-else class="dot" :style="{ background: a.color }"></span>
      <span class="name" :title="a.name">{{ a.label }}</span>
      <span class="time">{{ fmtDuration(a.seconds) }}</span>
      <div class="bar">
        <NProgress
          type="line"
          :percentage="a.pct"
          :show-indicator="false"
          :height="6"
          border-radius="3px"
          :color="a.color"
          :rail-color="rail"
        />
      </div>
    </button>
    <p v-if="!shown.length" class="empty">今天还没有记录</p>
    <p v-else-if="rest" class="more">还有 {{ rest }} 个应用没显示（显示条数可在设置里调）</p>
  </div>
</template>

<style scoped>
.ranking {
  display: flex;
  flex-direction: column;
  gap: 11px;
}

.row {
  width: 100%;
  border: 0;
  background: transparent;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  display: grid;
  /* 序号 / 图标 / 名称 / 时长 —— 图标列要够宽，否则会压到名称上 */
  grid-template-columns: 18px 26px 1fr auto;
  grid-template-rows: auto auto;
  column-gap: 12px;
  align-items: center;
  transition: transform var(--dur);
}

.row:hover {
  transform: translateX(2px);
}

.rank {
  grid-row: 1;
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

.iapp {
  grid-row: 1;
  grid-column: 2;
  justify-self: center;
  width: 18px;
  height: 18px;
  border-radius: 5px;
  object-fit: contain;
}

.dot {
  grid-row: 1;
  grid-column: 2;
  justify-self: center;
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.name {
  grid-row: 1;
  font-size: 13px;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.time {
  grid-row: 1;
  font-size: 12px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.bar {
  grid-column: 3 / 5;
  margin-top: 4px;
}

.empty {
  color: var(--text-faint);
  font-size: 13px;
  text-align: center;
  padding: 16px 0;
}

.more {
  margin: 2px 0 0;
  font-size: 11px;
  color: var(--text-faint);
  text-align: right;
}
</style>
