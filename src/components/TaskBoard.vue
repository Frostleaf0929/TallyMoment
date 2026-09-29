<script setup lang="ts">
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NCheckbox, NPopconfirm } from "naive-ui";
import type { Task } from "../types";

const props = defineProps<{ tasks: Task[] }>();
const emit = defineEmits<{ (e: "reload"): void; (e: "jump", taskId: number): void }>();

const editingId = ref<number | null>(null);
const editDraft = ref("");

const open = computed(() => props.tasks.filter((t) => !t.done));
const closed = computed(() => props.tasks.filter((t) => t.done));

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
}

async function saveEdit(t: Task) {
  const draft = editDraft.value.trim();
  editingId.value = null;
  if (!draft || draft === t.content) return;
  await invoke("task_update", {
    id: t.id,
    content: draft,
    priority: t.priority,
    dueTs: t.dueTs,
  });
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
      <input
        v-if="editingId === t.id"
        v-model="editDraft"
        class="edit"
        autofocus
        @keyup.enter="saveEdit(t)"
        @blur="saveEdit(t)"
      />
      <span
        v-else
        class="content clickable"
        :title="`${t.content}（点击到详细 · 事项查看坚持情况）`"
        @click="emit('jump', t.templateId ?? t.id)"
        @dblclick="startEdit(t)"
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
      <button v-if="!t.done" class="startbtn" :class="{ on: !!t.startTs }" :title="t.startTs ? '结束计时' : '开始做'" @click="toggleStart(t)">
        {{ t.startTs ? "计时中" : "开始" }}
      </button>
      <NPopconfirm @positive-click="remove(t)">
        <template #trigger>
          <NButton quaternary size="tiny" type="error">删除</NButton>
        </template>
        删除这条任务？
      </NPopconfirm>
    </div>
    <p v-if="!open.length" class="empty">没有进行中的任务，上面加一条吧</p>

    <p v-if="closed.length" class="done-head">已完成 · {{ closed.length }}</p>
    <div v-for="t in closed" :key="'d' + t.id" class="row done">
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

.edit {
  flex: 1;
  border: 1px solid var(--accent-border);
  background: var(--surface);
  color: var(--text);
  border-radius: 6px;
  padding: 3px 8px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
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
