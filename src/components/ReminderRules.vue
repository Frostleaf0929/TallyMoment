<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NInput, NPopconfirm, NSwitch } from "naive-ui";
import type { ReminderRule } from "../types";

const props = defineProps<{ rules: ReminderRule[] }>();
const emit = defineEmits<{ (e: "reload"): void }>();

// 编辑态：null = 无；-1 = 新建
const editingId = ref<number | null>(null);
const draft = ref({
  title: "",
  body: "",
  mode: "interval" as "interval" | "daily",
  intervalMinutes: 30,
  dailyTime: "",
  dailyTimes: [] as string[],
  sticky: false,
  cardDurationSec: 10,
  style: "card" as "card" | "fullscreen",
});

const err = ref("");

function openNew() {
  editingId.value = -1;
  err.value = "";
  draft.value = {
    title: "",
    body: "",
    mode: "interval",
    intervalMinutes: 30,
    dailyTime: "",
    dailyTimes: [],
    sticky: false,
    cardDurationSec: 10,
    style: "card",
  };
}

function openEdit(r: ReminderRule) {
  editingId.value = r.id;
  err.value = "";
  draft.value = {
    title: r.title,
    body: r.body,
    mode: r.mode,
    intervalMinutes: r.intervalMinutes ?? 30,
    dailyTime: "",
    dailyTimes: [...r.dailyTimes],
    sticky: r.sticky,
    style: (r.style || "card") as "card" | "fullscreen",
    cardDurationSec: r.cardDurationSec,
  };
}

function close() {
  editingId.value = null;
}

function addTime() {
  const t = draft.value.dailyTime.trim();
  if (t && !draft.value.dailyTimes.includes(t) && draft.value.dailyTimes.length < 8) {
    draft.value.dailyTimes.push(t);
    draft.value.dailyTimes.sort();
  }
  draft.value.dailyTime = "";
}

async function save() {
  err.value = "";
  const isNew = editingId.value === -1;
  const args = {
    title: draft.value.title,
    body: draft.value.body,
    mode: draft.value.mode,
    intervalMinutes: draft.value.mode === "interval" ? draft.value.intervalMinutes : null,
    dailyTimes: draft.value.mode === "daily" ? draft.value.dailyTimes : [],
    sticky: draft.value.sticky,
    cardDurationSec: draft.value.cardDurationSec,
    accentColor: null,
    style: draft.value.style,
  };
  try {
    await invoke(isNew ? "rule_add" : "rule_update", isNew ? args : { id: editingId.value, ...args });
    close();
    emit("reload");
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function toggle(r: ReminderRule, v: boolean) {
  await invoke("rule_set_enabled", { id: r.id, enabled: v });
  emit("reload");
}

async function remove(r: ReminderRule) {
  await invoke("rule_delete", { id: r.id });
  emit("reload");
}

const modeLabel = (r: ReminderRule) =>
  r.mode === "interval" ? `每 ${r.intervalMinutes} 分钟` : `定点 ${r.dailyTimes.join(" / ")}`;
</script>

<template>
  <div>
    <div class="toolbar">
      <span class="count">{{ rules.filter((r) => r.enabled).length }} 条启用中</span>
      <NButton size="small" type="primary" secondary @click="openNew">新建提醒</NButton>
    </div>

    <div v-for="r in rules" :key="r.id" class="rule" :class="{ off: !r.enabled }">
      <div class="line1">
        <span class="title" :title="r.title">{{ r.title }}</span>
        <span class="tag">{{ modeLabel(r) }}</span>
        <span class="tag">{{ r.sticky ? "卡片常驻" : `停留 ${r.cardDurationSec}s` }}</span>
        <NSwitch size="small" :value="r.enabled" @update:value="(v: boolean) => toggle(r, v)" />
        <NButton quaternary size="tiny" @click="openEdit(r)">编辑</NButton>
        <NPopconfirm @positive-click="remove(r)">
          <template #trigger>
            <NButton quaternary size="tiny" type="error">删除</NButton>
          </template>
          删除提醒「{{ r.title }}」？
        </NPopconfirm>
      </div>
      <p v-if="r.body" class="body">{{ r.body }}</p>
    </div>
    <p v-if="!rules.length" class="empty">还没有提醒规则，点右上角「新建提醒」开始</p>

    <!-- 内联编辑卡 -->
    <div v-if="editingId !== null" class="editor glass-card">
      <p class="et">{{ editingId === -1 ? "新建提醒" : "编辑提醒" }}</p>
      <NInput v-model:value="draft.title" size="small" placeholder="标题，如：护眼提醒" maxlength="50" />
      <NInput v-model:value="draft.body" size="small" type="textarea" placeholder="正文（可选）" :rows="2" />
      <div class="seg">
        <button class="seg-i" :class="{ on: draft.mode === 'interval' }" @click="draft.mode = 'interval'">
          时间间隔
        </button>
        <button class="seg-i" :class="{ on: draft.mode === 'daily' }" @click="draft.mode = 'daily'">
          每日定点
        </button>
      </div>
      <div v-if="draft.mode === 'interval'" class="frow">
        <span>每</span>
        <input v-model.number="draft.intervalMinutes" type="number" min="1" max="1440" class="num" />
        <span>分钟提醒一次（1~1440）</span>
      </div>
      <div v-else class="frow">
        <input v-model="draft.dailyTime" type="time" class="tp" />
        <NButton size="tiny" @click="addTime">添加</NButton>
        <span v-if="draft.dailyTimes.length" class="times">{{ draft.dailyTimes.join(" / ") }}</span>
      </div>
      <div class="frow">
        <span>停留</span>
        <input v-model.number="draft.cardDurationSec" type="number" min="3" max="600" class="num" />
        <span>秒</span>
        <label class="stk"><input v-model="draft.sticky" type="checkbox" /> 卡片常驻</label>
      </div>
      <p v-if="err" class="err">{{ err }}</p>
      <div class="btns">
        <NButton size="small" @click="close">取消</NButton>
        <NButton size="small" type="primary" @click="save">{{ editingId === -1 ? "创建" : "保存" }}</NButton>
      </div>
    </div>
  </div>
</template>

<style scoped>
.seg {
  display: inline-flex;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  padding: 4px;
  background: var(--surface);
}

.seg-item {
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 12.5px;
  font-family: inherit;
  padding: 5px 12px;
  border-radius: var(--r-sm);
  cursor: pointer;
}

.seg-item.active {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.count {
  font-size: 12px;
  color: var(--text-muted);
}

.rule {
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-md);
  padding: 10px 12px;
  margin-bottom: 8px;
}

.rule.off {
  opacity: 0.5;
  border-style: dashed;
}

.line1 {
  display: flex;
  align-items: center;
  gap: 10px;
}

.title {
  flex: 1;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tag {
  flex: none;
  font-size: 10px;
  color: var(--accent-text);
  background: var(--accent-soft);
  border-radius: var(--r-full);
  padding: 2px 9px;
}

.body {
  margin: 6px 0 0;
  font-size: 12px;
  color: var(--text-muted);
}

.empty {
  font-size: 12px;
  color: var(--text-faint);
}

.editor {
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 12px;
}

.et {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}

.seg {
  display: inline-flex;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  padding: 3px;
  background: var(--surface);
  width: fit-content;
}

.seg-i {
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 12px;
  font-family: inherit;
  padding: 6px 12px;
  border-radius: var(--r-sm);
  cursor: pointer;
}

.seg-i.on {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.frow {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-muted);
  flex-wrap: wrap;
}

.num {
  width: 70px;
  border: 1px solid var(--border-strong);
  background: var(--surface);
  color: var(--text);
  border-radius: 6px;
  padding: 4px 6px;
  font-size: 12px;
  font-family: inherit;
  outline: none;
}

.tp {
  border: 1px solid var(--border-strong);
  background: var(--surface);
  color: var(--text);
  border-radius: 6px;
  padding: 4px 6px;
  font-size: 12px;
  font-family: inherit;
  outline: none;
}

.times {
  color: var(--accent-text);
}

.stk {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
}

.err {
  margin: 0;
  font-size: 12px;
  color: var(--danger);
}

.btns {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

button {
  font-family: inherit;
}
</style>
