<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/** 原子岛：常驻置顶胶囊（终末地风格预设）。
 *  数据由本页每秒向后端拉取（窗口隐藏时不渲染，不占资源）。 */
interface NextTask {
  id: number;
  content: string;
  dueTs: number;
}
interface IslandData {
  focusName: string | null;
  focusSec: number;
  doneToday: number;
  paused: boolean;
  nextTask: NextTask | null;
  tasks: NextTask[];
}

const data = ref<IslandData | null>(null);
const nowSec = ref(Math.floor(Date.now() / 1000));
const expanded = ref(false);
let timer: number | undefined;
let clock: number | undefined;

async function refresh() {
  try {
    data.value = await invoke<IslandData>("island_data");
  } catch {
    /* 后端忙就等下一秒 */
  }
}

function toggleExpand() {
  expanded.value = !expanded.value;
  void invoke("island_set_expanded", { expanded: expanded.value });
}

async function done(t: NextTask) {
  try {
    await invoke("task_set_done", { id: t.id, done: true });
    await refresh();
  } catch {
    /* 忽略 */
  }
}

function fmtDelta(sec: number): string {
  const s = Math.max(0, sec);
  if (s >= 3600) return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
  if (s >= 60) return `${Math.floor(s / 60)}m ${s % 60}s`;
  return `${s}s`;
}

function fmtDue(dueTs: number): string {
  return fmtDelta(dueTs - nowSec.value);
}

const focusLabel = computed(() => {
  if (!data.value) return "…";
  if (data.value.paused) return "已暂停";
  return data.value.focusName ? `${data.value.focusName} · ${fmtDelta(data.value.focusSec)}` : "休息中";
});

const nextLabel = computed(() => {
  const t = data.value?.nextTask;
  return t ? `${t.content} · ${fmtDue(t.dueTs)}` : "没有排期中的任务";
});

onMounted(() => {
  void invoke("island_ready");
  void refresh();
  timer = window.setInterval(refresh, 1000);
  clock = window.setInterval(() => (nowSec.value = Math.floor(Date.now() / 1000)), 1000);
});
onUnmounted(() => {
  window.clearInterval(timer);
  window.clearInterval(clock);
});
</script>

<template>
  <div class="island" :class="{ expanded }">
    <!-- 胶囊条：中部空白区可拖动，右侧按钮展开/收起 -->
    <div class="bar" data-tauri-drag-region>
      <span class="mark"></span>
      <span class="focus" data-tauri-drag-region :class="{ off: data?.paused }">{{ focusLabel }}</span>
      <span class="div" data-tauri-drag-region></span>
      <span class="next" data-tauri-drag-region :title="data?.nextTask?.content">
        <span class="tag">下个</span>{{ nextLabel }}
      </span>
      <span class="done" data-tauri-drag-region>
        <b class="num">{{ data?.doneToday ?? 0 }}</b> 今日
      </span>
      <button class="tg" :title="expanded ? '收起' : '展开待办'" @click="toggleExpand">
        <svg width="10" height="10" viewBox="0 0 12 12" aria-hidden="true" :class="{ flip: expanded }">
          <path d="M2 4l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </button>
    </div>

    <!-- 展开区：待办列表（可直接勾掉） -->
    <div v-if="expanded" class="tasks">
      <p class="thead">待办 · 按截止排序</p>
      <div v-for="t in data?.tasks ?? []" :key="t.id" class="trow">
        <button class="ring" title="标记完成" @click="done(t)"></button>
        <span class="tname" :title="t.content">{{ t.content }}</span>
        <span class="tdue num">{{ fmtDue(t.dueTs) }}</span>
      </div>
      <p v-if="!(data?.tasks ?? []).length" class="tempty">没有排期中的任务</p>
    </div>
  </div>
</template>

<style scoped>
.island {
  height: 100vh;
  background: #0f1013;
  color: #e6e4dc;
  overflow: hidden;
  user-select: none;
  font-family: "Segoe UI Variable", "Segoe UI", "Microsoft YaHei UI", system-ui, sans-serif;
}

/* 胶囊条：斜切角（终末地风），琥珀黄细节 */
.bar {
  height: 100vh;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 12px 0 16px;
  clip-path: polygon(16px 0, 100% 0, 100% calc(100% - 16px), calc(100% - 16px) 100%, 0 100%, 0 16px);
  background: linear-gradient(135deg, #17181d, #101114);
  box-shadow: inset 0 0 0 1px rgba(232, 179, 75, 0.22);
  cursor: default;
  min-width: 0;
}

.mark {
  flex: none;
  width: 10px;
  height: 10px;
  background: #e8b34b;
  transform: rotate(45deg);
  box-shadow: 0 0 8px rgba(232, 179, 75, 0.5);
}

.focus {
  flex: none;
  max-width: 34%;
  font-size: 12.5px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.focus.off {
  color: #8b8a82;
}

.div {
  flex: none;
  width: 1px;
  height: 18px;
  background: rgba(232, 179, 75, 0.28);
}

.next {
  flex: 1;
  min-width: 0;
  font-size: 12px;
  color: #cfcdc4;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tag {
  display: inline-block;
  margin-right: 6px;
  padding: 0 6px;
  font-size: 10px;
  color: #e8b34b;
  border: 1px solid rgba(232, 179, 75, 0.4);
  line-height: 1.5;
}

.done {
  flex: none;
  font-size: 11.5px;
  color: #8b8a82;
}

.num {
  font-variant-numeric: tabular-nums;
  color: #e8b34b;
  font-weight: 700;
}

.tg {
  flex: none;
  width: 22px;
  height: 22px;
  border: 0;
  border-radius: 4px;
  background: rgba(232, 179, 75, 0.14);
  color: #e8b34b;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.tg svg {
  transition: transform 0.18s ease;
}

.tg svg.flip {
  transform: rotate(180deg);
}

/* 展开区 */
.tasks {
  padding: 4px 16px 14px;
  max-height: calc(100vh - 56px);
  overflow-y: auto;
}

.thead {
  margin: 6px 0 4px;
  font-size: 10.5px;
  letter-spacing: 0.18em;
  color: #8b8a82;
}

.trow {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 4px;
  border-bottom: 1px solid rgba(232, 179, 75, 0.12);
}

.ring {
  flex: none;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 1.5px solid rgba(232, 179, 75, 0.55);
  background: transparent;
  cursor: pointer;
  padding: 0;
}

.ring:hover {
  background: rgba(232, 179, 75, 0.25);
}

.tname {
  flex: 1;
  min-width: 0;
  font-size: 12.5px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tdue {
  flex: none;
  font-size: 11.5px;
  font-weight: 600;
}

.tempty {
  margin: 8px 0 0;
  font-size: 12px;
  color: #8b8a82;
}
</style>
