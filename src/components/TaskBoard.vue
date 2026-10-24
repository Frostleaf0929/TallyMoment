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

/* ---------- 重复任务分组：模板 + 每日实例合并成一行，历史不再刷屏 ---------- */
interface Group {
  key: string;
  /** 模板行（模板被删后为 null，只剩历史实例） */
  template: Task | null;
  content: string;
  priority: number;
  repeatMode: string;
  /** 已停止（或模板已删）：不再生成以后实例，历史保留 */
  stopped: boolean;
  stopLabel: string;
  todayInst: Task | null;
  /** 该条跳详情用的 id（模板优先） */
  jumpId: number;
  doneCount: number;
  /** 过期未做的旧实例（可一键清理，不影响已完成记录） */
  staleIds: number[];
  remindStyle: string;
  remindIntervalMin: number;
}

const todayStr = () => new Date().toISOString().slice(0, 10);
const dayOf = (t: Task) =>
  new Date((t.dueTs ?? t.createdTs) * 1000).toISOString().slice(0, 10);

const groups = computed<Group[]>(() => {
  const tplRows = props.tasks.filter((t) => t.repeatMode && !t.templateId);
  const byTpl = new Map<number, Task[]>();
  for (const t of props.tasks) {
    if (t.templateId != null) {
      if (!byTpl.has(t.templateId)) byTpl.set(t.templateId, []);
      byTpl.get(t.templateId)!.push(t);
    }
  }
  const today = todayStr();
  const out: Group[] = [];
  for (const tpl of tplRows) {
    const insts = byTpl.get(tpl.id) ?? [];
    const todayInst =
      insts.find((i) => dayOf(i) === today) ?? (dayOf(tpl) === today ? tpl : null);
    const stale = insts.filter((i) => !i.done && dayOf(i) !== today);
    out.push({
      key: `t${tpl.id}`,
      template: tpl,
      content: tpl.content,
      priority: tpl.priority,
      repeatMode: tpl.repeatMode,
      stopped: tpl.repeatStopped,
      stopLabel: tpl.repeatStopped ? "已停止" : "",
      todayInst,
      jumpId: tpl.id,
      doneCount: insts.filter((i) => i.done).length + (tpl.done ? 1 : 0),
      staleIds: stale.map((s) => s.id),
      remindStyle: tpl.remindStyle,
      remindIntervalMin: tpl.remindIntervalMin,
    });
  }
  // 模板已删的孤儿实例：聚成一行展示历史，避免刷屏；不再有"以后"，等同已停止
  for (const [tid, insts] of byTpl) {
    if (tplRows.some((t) => t.id === tid)) continue;
    const undone = insts.filter((i) => !i.done);
    out.push({
      key: `o${tid}`,
      template: null,
      content: insts[0]?.content ?? "(已删除的重复任务)",
      priority: insts[0]?.priority ?? 1,
      repeatMode: "",
      stopped: true,
      stopLabel: "已删除",
      todayInst: undone.find((i) => dayOf(i) === today) ?? null,
      jumpId: insts[0]?.templateId ?? tid,
      doneCount: insts.filter((i) => i.done).length,
      staleIds: undone.filter((i) => dayOf(i) !== today).map((i) => i.id),
      remindStyle: insts[0]?.remindStyle ?? "",
      remindIntervalMin: insts[0]?.remindIntervalMin ?? 0,
    });
  }
  return out;
});

const openGroups = computed(() =>
  groups.value
    .filter((g) => (g.todayInst ? !g.todayInst.done : true))
    .filter((g) => g.todayInst || g.staleIds.length || !g.doneCount || g.template)
    .sort((a, b) => {
      const rank = (g: Group) => (g.todayInst ? 0 : g.staleIds.length ? 1 : g.stopped ? 3 : 2);
      return rank(a) - rank(b);
    })
);

const oneOffOpen = computed(() =>
  props.tasks.filter((t) => !t.repeatMode && !t.templateId && !t.done)
);
const oneOffDone = computed(() =>
  props.tasks.filter((t) => !t.repeatMode && !t.templateId && t.done)
);
const closed = computed(() => oneOffDone.value);
const closedShown = computed(() =>
  showAllDone.value ? closed.value : closed.value.slice(0, DONE_PREVIEW)
);
/** 已完成区的重复任务历史行（所有有完成记录的组） */
const doneGroups = computed(() => groups.value.filter((g) => g.doneCount > 0));

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

/** 开始/结束计时（重复任务落在今天的实例上） */
async function toggleStart(t: Task) {
  await invoke("task_set_started", { id: t.id, ts: t.startTs ? null : Math.floor(Date.now() / 1000) });
  emit("reload");
}

async function toggleGroupStart(g: Group) {
  if (!g.todayInst) return;
  await toggleStart(g.todayInst);
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

function startGroupEdit(g: Group) {
  const t = g.template ?? g.todayInst;
  if (t) startEdit(t);
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

/** 重复任务勾选：有今天实例切今天；没有就补一张今天的卡（habit_toggle） */
async function toggleGroup(g: Group, v: boolean) {
  if (g.todayInst) {
    await invoke("task_set_done", { id: g.todayInst.id, done: v });
  } else if (g.template) {
    await invoke("habit_toggle", { taskId: g.template.id, date: todayStr() });
  }
  emit("reload");
}

async function remove(t: Task) {
  await invoke("task_delete", { id: t.id });
  emit("reload");
}

/** 停止/恢复重复任务：只影响以后生成，历史记录保留 */
async function toggleStop(g: Group) {
  if (!g.template) return;
  await invoke("task_stop_repeat", { id: g.template.id, stopped: !g.template.repeatStopped });
  emit("reload");
}

/** 清理过期未做的旧实例（软删除，已完成记录不受影响） */
const clearing = ref(false);
async function clearStale(g: Group) {
  clearing.value = true;
  try {
    for (const id of g.staleIds) await invoke("task_delete", { id });
    emit("reload");
  } finally {
    clearing.value = false;
  }
}
</script>

<template>
  <div>
    <!-- 一次性任务（未完成） -->
    <div v-for="t in oneOffOpen" :key="t.id" class="row">
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
        删除这条任务？（已完成记录保留）
      </NPopconfirm>
    </div>
    <p v-if="!oneOffOpen.length && !openGroups.length" class="empty">没有进行中的任务，上面加一条吧</p>

    <!-- 重复任务：一条一行（每天生成的实例合并进这里，历史在下面/习惯追踪里看） -->
    <div
      v-for="g in openGroups"
      :key="g.key"
      class="row"
      :class="{ dim: !g.todayInst || (g.todayInst && g.todayInst.done) }"
    >
      <NCheckbox
        v-if="g.todayInst || g.template"
        size="small"
        :checked="!!g.todayInst?.done"
        :disabled="g.stopped && !g.todayInst"
        title="勾选 = 完成今天这一次；没有今天的实例时勾选会补一张今天的卡"
        @update:checked="(v: boolean) => toggleGroup(g, v)"
      />
      <span class="prio" :class="priorityClass(g.priority)">{{ priorityLabel(g.priority) }}</span>
      <div v-if="editingId === (g.template ?? g.todayInst)?.id" class="editwrap">
        <input v-model="editDraft" class="edit" autofocus @keyup.enter="saveEdit((g.template ?? g.todayInst)!)" />
        <div class="editrow">
          <select v-model="editStyle" class="esel">
            <option v-for="o in STYLE_OPTS" :key="o.value" :value="o.value">{{ o.label }}</option>
          </select>
          <select v-model="editInterval" class="esel">
            <option v-for="o in INTERVAL_OPTS" :key="o.value" :value="o.value">{{ o.label }}</option>
          </select>
          <button class="esave" @mousedown.prevent="saveEdit((g.template ?? g.todayInst)!)">保存</button>
        </div>
        <p class="apps-label">编辑重复任务会同步到以后未完成的每一天（已完成的历史不变）</p>
      </div>
      <template v-else>
        <span class="content clickable" title="点击跳转详情" @click="emit('jump', g.jumpId)">
          {{ g.content }}
        </span>
        <span v-if="g.todayInst?.startTs && !g.todayInst?.done" class="spent">计时中</span>
        <span v-if="g.todayInst" class="due" :class="{ overdue: !g.todayInst.done && (g.todayInst.dueTs ?? 0) * 1000 < Date.now() }">
          {{ dueLabel(g.todayInst) }}
        </span>
        <span v-else-if="!g.stopped" class="due">今天无安排</span>
        <span v-if="g.repeatMode" class="rep" :title="g.stopped ? '已停止生成以后的任务' : '重复任务，每天只占一行'">
          {{ repeatLabel(g.repeatMode) }}{{ g.stopped ? " · 已停止" : "" }}
        </span>
        <span v-else-if="g.stopped" class="rep no">{{ g.stopLabel }}</span>
        <span v-if="g.doneCount" class="spent" title="历史完成次数（点内容跳详情看每天情况）">
          已完成 {{ g.doneCount }} 次
        </span>
        <span v-if="g.staleIds.length" class="stale" title="过期未做的旧实例，清理后已完成记录不受影响">
          {{ g.staleIds.length }} 天未做
        </span>
        <span v-if="g.remindIntervalMin > 0" class="rep iv">{{ intervalLabel(g.remindIntervalMin) }}</span>
        <span v-else-if="g.remindStyle === 'fullscreen'" class="rep fs">全屏提醒</span>
        <span v-else-if="g.remindStyle === 'none'" class="rep no">不提醒</span>
      </template>
      <button
        v-if="g.todayInst && !g.todayInst.done"
        class="startbtn"
        :class="{ on: !!g.todayInst.startTs }"
        :title="g.todayInst.startTs ? '结束计时' : '开始做今天这一次'"
        @click="toggleGroupStart(g)"
      >
        {{ g.todayInst.startTs ? "计时中" : "开始" }}
      </button>
      <NPopconfirm v-if="g.staleIds.length" @positive-click="clearStale(g)">
        <template #trigger>
          <button class="startbtn editbtn" :disabled="clearing">清理过期</button>
        </template>
        把 {{ g.staleIds.length }} 天过期未做的旧实例删掉？（已完成的历史记录不受影响）
      </NPopconfirm>
      <button v-if="g.template" class="startbtn editbtn" title="编辑内容与提醒（同步到未完成的实例）" @click="startGroupEdit(g)">编辑</button>
      <button v-if="g.template" class="startbtn stopbtn" @click="toggleStop(g)">
        {{ g.template.repeatStopped ? "恢复重复" : "停止重复" }}
      </button>
      <NPopconfirm v-if="g.template" @positive-click="remove(g.template!)">
        <template #trigger>
          <NButton quaternary size="tiny" type="error">删除</NButton>
        </template>
        停止生成以后的实例并隐藏这条重复任务？已完成的历史记录会保留在统计与习惯追踪里。
      </NPopconfirm>
    </div>

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

    <!-- 重复任务历史：合并计数，不逐天列出（逐天在习惯追踪/详情页） -->
    <template v-if="doneGroups.length">
      <p class="done-head">重复任务历史</p>
      <div v-for="g in doneGroups" :key="'g' + g.key" class="row done">
        <span class="content strikethrough clickable" title="点击跳转详情看每天情况" @click="emit('jump', g.jumpId)">
          {{ g.content }}
        </span>
        <span v-if="g.repeatMode" class="rep">{{ repeatLabel(g.repeatMode) }}{{ g.stopped ? " · 已停止" : "" }}</span>
        <span v-else class="rep no">{{ g.stopLabel }}</span>
        <span class="spent">已完成 {{ g.doneCount }} 次</span>
        <span v-if="g.staleIds.length" class="stale">{{ g.staleIds.length }} 天未做</span>
        <NPopconfirm v-if="g.staleIds.length" @positive-click="clearStale(g)">
          <template #trigger>
            <button class="startbtn editbtn" :disabled="clearing">清理过期</button>
          </template>
          把 {{ g.staleIds.length }} 天过期未做的旧实例删掉？（已完成的历史记录不受影响）
        </NPopconfirm>
      </div>
    </template>
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

/* 今天无安排 / 已停止的重复任务行：弱化显示 */
.row.dim {
  opacity: 0.72;
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

/* 过期未做提示：暖色但不刺眼 */
.stale {
  font-size: 10px;
  color: var(--warn);
  background: var(--warn-soft);
  border-radius: var(--r-full);
  padding: 1px 8px;
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

.startbtn.stopbtn {
  color: var(--warn);
  border-color: var(--warn);
}

.startbtn:disabled {
  opacity: 0.5;
  cursor: default;
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
