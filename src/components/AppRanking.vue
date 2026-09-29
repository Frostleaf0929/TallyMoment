<script setup lang="ts">
import { computed, ref, watchEffect } from "vue";
import { NProgress } from "naive-ui";
import { invoke } from "@tauri-apps/api/core";
import type { AppUsage } from "../types";
import { colorFor } from "../lib/colors";
import { fmtDuration } from "../lib/format";
import { chartColors } from "../lib/chartColors";
import { appsTopN, isLight } from "../lib/uiState";
import { appColorMode, appNameEnglish } from "../lib/appearance";
import { iconColor, iconUrl, requestIcons } from "../lib/appIcons";

const emit = defineEmits<{ (e: "pick", name: string): void; (e: "renamed"): void }>();

/* ---------- 应用改名：把 floral-notepaper.exe 这类进程名改成"花笺"这样的真实软件名 ---------- */
const editing = ref("");
const editDraft = ref("");

function startRename(key: string, label: string) {
  editing.value = key;
  // 显示名去掉 .exe 后与进程名相同时视为"没改过"，清空输入（placeholder 显示进程名）
  editDraft.value = label === key.replace(/\.exe$/i, "") ? "" : label;
}

async function saveRename(key: string) {
  const draft = editDraft.value.trim();
  editing.value = "";
  try {
    await invoke("app_rename", { name: key, displayName: draft });
    emit("renamed");
  } catch {
    /* 忽略 */
  }
}

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
      <span class="name" :title="a.name">
        <input
          v-if="editing === a.key"
          v-model="editDraft"
          class="rn"
          :placeholder="a.name.replace(/\.exe$/i, '')"
          @keyup.enter="saveRename(a.key)"
          @blur="saveRename(a.key)"
          @click.stop
          @keydown.esc="editing = ''"
        />
        <template v-else>{{ a.label }}</template>
      </span>
      <span class="time">{{ fmtDuration(a.seconds) }}</span>
      <span class="rnbtn" title="改成实际软件名（清空 = 恢复进程名）" @click.stop="startRename(a.key, a.label)">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M4 20h4L19 9l-4-4L4 16v4zM13 6l4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </span>
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
  /* 序号 / 图标 / 名称 / 时长 / 改名 —— 图标列要够宽，否则会压到名称上 */
  grid-template-columns: 18px 26px 1fr auto 18px;
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
  grid-column: 3 / 6;
  margin-top: 4px;
}

.rnbtn {
  grid-row: 1;
  grid-column: 5;
  justify-self: end;
  width: 18px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 5px;
  color: var(--text-faint);
  opacity: 0;
  transition: opacity var(--dur), color var(--dur), background var(--dur);
}

.row:hover .rnbtn {
  opacity: 1;
}

.rnbtn:hover {
  color: var(--accent-text);
  background: var(--surface-hover);
}

.rnbtn svg {
  width: 12px;
  height: 12px;
}

.rn {
  width: 100%;
  border: 1px solid var(--accent-border);
  background: var(--surface);
  color: var(--text);
  border-radius: 6px;
  padding: 2px 8px;
  font-size: 12.5px;
  font-family: inherit;
  outline: none;
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
