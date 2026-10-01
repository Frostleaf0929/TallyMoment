<script setup lang="ts">
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NCheckbox, NPopconfirm } from "naive-ui";
import type { Task } from "../types";

const props = defineProps<{ tasks: Task[] }>();
const emit = defineEmits<{ (e: "reload"): void; (e: "jump", taskId: number): void }>();

const showAllDone = ref(false);
const DONE_PREVIEW = 12;
const editingId = ref<number | null>(null);
const editDraft = ref("");
const editStyle = ref("");
const editInterval = ref(0);
const editApps = ref<string[]>([]);
const editAppInput = ref("");
const suggestApps = ref<string[]>([]);

const STYLE_OPTS = [
  { label: "到期提醒：默认卡片", value: "" },
  { label: "到期提醒：卡片", value: "card" },
  { label: "到期提醒：全屏", value: "fullscreen" },
  { label: "到期提醒：关闭", value: "none" },
];
const INTERVAL_OPTS = [
  { label: "不间隔提醒", value: 0 },
  { label: "每 5 分钟确认", value: 5 },
  { label: "每 15 分钟确认", value: 15 },
  { label: "每 30 分钟确认", value: 30 },
  { label: "每 60 分钟确认", value: 60 },
];
const intervalLabel = (m: number) =>
  ({ 5: "每5分确认", 15: "每15分确认", 30: "每30分确认", 60: "每60分确认" })[m] ?? "";

const open = computed(() => props.tasks.filter((t) => !t.done));
const closed = computed(() => props.tasks.filter((t) => t.done));
const closedShown = computed(() =>
  showAllDone.value ? closed.value : closed.value.slice(0, DONE_PREVIEW)
);

const priorityLabel = (p: number) => (p === 2 ? "高" : p === 0 ? "低" : "中");
const priorityClass = (p: number) => (p === 2 ? "high" : p === 0 ? "low" : "mid");

/** 固定事项标签 */
const repeatLabel = (mode: string): string =>
  ({ daily: "每天", weekly: "每周", monthly: "每月", yearly: "每年" })[mode] ?? "";

/** 完成用时：优先 开始→完成，其次 创建→完成 */
const spent = (t: Task): string => {
  if (!t.done || !t.doneTs) return "";
  const from = t.startTs ?? t.createdTs;
  const mins = Math.round((t.doneTs - from) / 60);
  return mins < 1 ? "不到 1 分钟" : `${mins} 分钟`;
};

/** 开始/结束计时 */
async function toggleStart(t: Task) {
  await invoke("task_set_started", { id: t.id, ts: t.startTs ? null : Math.floor(Date.now() / 1000) });
  emit("reload");
}

const dueLabel = (t: Task): string => {
  if (!t.dueTs) return "";
  const d = new Date(t.dueTs * 1000);
  const hhmm = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  const today = new Date().toDateString() === d.toDateString();
  const md = `${d.getMonth() + 1}/${d.getDate()}`;
  const overdue = !t.done && t.dueTs * 1000 < Date.now();
  return `${today ? "今天" : md} ${hhmm}${overdue ? " · 已过期" : ""}`;
};

function startEdit(t: Task) {
  editingId.value = t.id;
  editDraft.value = t.content;
  editStyle.value = t.remindStyle ?? "";
  editInterval.value = t.remindIntervalMin ?? 0;
  editApps.value = [...(t.relatedApps ?? [])];
  editAppInput.value = "";
  suggestApps.value = [];
  // 从任务追踪时间窗推荐高频应用（有开始时间才查）
  if (t.startTs) {
    invoke<[string, number][]>("task_suggest_apps", { id: t.id })
      .then((r) => {
        suggestApps.value = r
          .map((x) => x[0])
          .filter((x) => !editApps.value.includes(x))
          .slice(0, 5);
      })
      .catch(() => {});
  }
}

function removeApp(a: string) {
  editApps.value = editApps.value.filter((x) => x !== a);
}

function addApp(a: string) {
  const v = a.trim().toLowerCase();
  if (v && !editApps.value.includes(v)) editApps.value.push(v);
  suggestApps.value = suggestApps.value.filter((x) => x !== v);
  editAppInput.value = "";
}

async function saveEdit(t: Task) {
  const draft = editDraft.value.trim();
  editingId.value = null;
  if (!draft) return;
  await invoke("task_update", {
    id: t.id,
    content: draft,
    priority: t.priority,
    dueTs: t.dueTs,
    remindStyle: editStyle.value,
    remindIntervalMin: editInterval.value,
  });
  await invoke("task_set_related_apps", { id: t.id, apps: editApps.value });
  emit("reload");
}

async function toggle(t: Task, v: boolean) {
  await invoke("task_set_done", { id: t.id, done: v });
  emit("reload");
}

async function remove(t: Task) {
  await invoke("task_delete", { id: t.id });
  emit("reload");
}
</script>

<template>
  <div>
    <div v-for="t in open" :key="t.id" class="row">
      <NCheckbox
        size="small"
        :checked="t.done"
        @update:checked="(v: boolean) => toggle(t, v)"
      />
      <span class="prio" :class="priorityClass(t.priority)">{{ priorityLabel(t.priority) }}</span>
      <div v-if="editingId === t.id" class="editwrap">
        <input
          v-model="editDraft"
          class="edit"
          autofocus
          @keyup.enter="saveEdit(t)"
        />
        <div class="editrow appsrow">
          <span class="apps-label" title="用于把心流时段自动归属到这个任务">相关应用</span>
          <span
            v-for="a in editApps"
            :key="a"
            class="appchip"
            title="点击移除"
            @click="removeApp(a)"
          >{{ a.replace(".exe", "") }} ✕</span>
          <button
            v-for="a in suggestApps"
            :key="'s' + a"
            class="appchip sug"
            :title="'追踪期高频应用，点击添加'"
            @click="addApp(a)"
          >+ {{ a.replace(".exe", "") }}</button>
          <input
            v-model="editAppInput"
            class="eapp"
            placeholder="手动加应用名"
            @keyup.enter="addApp(editAppInput)"
          />
        </div>
        <div class="editrow">
          <select v-model="editStyle" class="esel">
            <option v-for="o in STYLE_OPTS" :key="o.value" :value="o.value">{{ o.label }}</option>
          </select>
          <select v-model="editInterval" class="esel">
            <option v-for="o in INTERVAL_OPTS" :key="o.value" :value="o.value">{{ o.label }}</option>
          </select>
          <button class="esave" @mousedown.prevent="saveEdit(t)">保存</button>
        </div>
      </div>
      <span
        v-else
        class="content clickable"
        title="点击跳转详情"
        @click="emit('jump', t.templateId ?? t.id)"
      >
        {{ t.content }}
      </span>
      <span v-if="spent(t)" class="spent">{{ spent(t) }}</span>
      <span v-if="t.dueTs" class="due" :class="{ overdue: !t.done && t.dueTs * 1000 < Date.now() }">
        {{ dueLabel(t) }}
      </span>
      <span v-if="t.repeatMode" class="rep" :title="t.templateId ? '固定事项生成的今日实例' : '固定事项模板'">
        {{ repeatLabel(t.repeatMode) }}{{ t.templateId ? "" : "·模板" }}
      </span>
      <span v-if="t.remindIntervalMin > 0" class="rep iv" title="计时中每 N 分钟弹卡确认一次">
        {{ intervalLabel(t.remindIntervalMin) }}
      </span>
      <span v-else-if="t.remindStyle === 'fullscreen'" class="rep fs" title="到期全屏提醒">全屏提醒</span>
      <span v-else-if="t.remindStyle === 'none'" class="rep no" title="到期不提醒">不提醒</span>
      <button v-if="!t.done" class="startbtn" :class="{ on: !!t.startTs }" :title="t.startTs ? '结束计时' : '开始做'" @click="toggleStart(t)">
        {{ t.startTs ? "计时中" : "开始" }}
      </button>
      <button v-if="!t.done" class="startbtn editbtn" title="编辑内容与提醒" @click="startEdit(t)">编辑</button>
      <NPopconfirm @positive-click="remove(t)">
        <template #trigger>
          <NButton quaternary size="tiny" type="error">删除</NButton>
        </template>
        删除这条任务？
      </NPopconfirm>
    </div>
    <p v-if="!open.length" class="empty">没有进行中的任务，上面加一条吧</p>

    <p v-if="closed.length" class="done-head">
      已完成 · {{ closed.length }}（显示{{ showAllDone ? "全部" : `最近 ${DONE_PREVIEW} 条` }})
      <button
        v-if="closed.length > DONE_PREVIEW"
        class="startbtn"
        @click="showAllDone = !showAllDone"
      >
        {{ showAllDone ? "收起" : "展开全部" }}
      </button>
    </p>
    <div v-for="t in closedShown" :key="'d' + t.id" class="row done">
      <NCheckbox
        size="small"
        :checked="t.done"
        @update:checked="(v: boolean) => toggle(t, v)"
      />
      <span class="content strikethrough">{{ t.content }}</span>
      <NPopconfirm @positive-click="remove(t)">
        <template #trigger>
          <NButton quaternary size="tiny" type="error">删除</NButton>
        </template>
        删除这条任务？
      </NPopconfirm>
    </div>
  </div>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 10px;
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-md);
  margin-bottom: 8px;
}

.row.done {
  opacity: 0.55;
}

.prio {
  flex: none;
  font-size: 10px;
  padding: 1px 8px;
  border-radius: var(--r-full);
}

.prio.high {
  color: var(--danger);
  background: var(--danger-soft);
}

.prio.mid {
  color: var(--warn);
  background: var(--warn-soft);
}

.prio.low {
  color: var(--text-muted);
  background: var(--surface);
}

.content {
  flex: 1;
  font-size: 13px;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: text;
}

.content.clickable:hover {
  color: var(--accent-text);
  text-decoration: underline;
  text-underline-offset: 3px;
}

.content.strikethrough {
  text-decoration: line-through;
}

.editwrap {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.edit {
  width: 100%;
  border: 1px solid var(--accent-border);
  background: var(--surface);
  color: var(--text);
  border-radius: 6px;
  padding: 3px 8px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
}

.editrow {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

/* 相关应用编辑器（心流归属） */
.appsrow {
  margin-top: 4px;
}
.apps-label {
  flex: none;
  font-size: 11px;
  color: var(--text-muted, #888);
}
.appchip {
  font-size: 11px;
  padding: 1px 8px;
  border: 1px solid rgba(128, 128, 128, 0.35);
  border-radius: 999px;
  cursor: pointer;
  color: var(--text, #ddd);
  white-space: nowrap;
}
.appchip:hover {
  border-color: var(--text, #ddd);
}
.appchip.sug {
  border-style: dashed;
  opacity: 0.75;
}
.eapp {
  width: 110px;
  font-size: 11px;
  background: none;
  border: 1px dashed rgba(128, 128, 128, 0.35);
  border-radius: 6px;
  color: var(--text, #ddd);
  padding: 2px 6px;
}

.esel {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  border-radius: 6px;
  font-size: 11px;
  font-family: inherit;
  padding: 2px 4px;
  max-width: 150px;
}

.esave {
  border: 1px solid var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent-text);
  border-radius: var(--r-full);
  font-size: 10px;
  font-family: inherit;
  padding: 2px 10px;
  cursor: pointer;
}

.rep.iv {
  color: var(--warn);
  background: var(--warn-soft);
}

.rep.fs {
  color: var(--danger);
  background: var(--danger-soft);
}

.rep.no {
  color: var(--text-faint);
  background: var(--surface);
}

.spent {
  font-size: 10px;
  color: var(--text-faint);
  flex: none;
}

.rep {
  font-size: 10px;
  color: var(--accent-text);
  background: var(--accent-soft);
  border-radius: var(--r-full);
  padding: 1px 8px;
  flex: none;
}

.startbtn {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-faint);
  border-radius: var(--r-full);
  font-size: 10px;
  font-family: inherit;
  padding: 1px 8px;
  cursor: pointer;
  flex: none;
}

.startbtn.editbtn {
  color: var(--accent-text);
  border-color: var(--accent-border);
}

.startbtn.on {
  color: var(--good);
  border-color: var(--good);
  background: var(--good-soft);
}

.due {
  flex: none;
  font-size: 11px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.due.overdue {
  color: var(--danger);
}

.empty,
.done-head {
  font-size: 12px;
  color: var(--text-faint);
  margin: 6px 0;
}
</style>
