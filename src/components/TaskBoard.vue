<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { NButton, NCheckbox, NPopconfirm } from "naive-ui";
import type { Task } from "../types";

const props = defineProps<{ tasks: Task[] }>();
const emit = defineEmits<{ (e: "reload"): void; (e: "jump", taskId: number): void }>();

const DONE_PREVIEW = 12;
const showAllDone = ref(false);
const listEl = ref<HTMLElement | null>(null);

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
  jumpId: number;
  doneCount: number;
  /** 过期未做的旧实例（可在表单里一键清理，不影响已完成记录） */
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
  // 模板已删的孤儿实例：聚成一行展示历史；不再有"以后"，等同已删除
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

const oneOffOpen = computed(() =>
  props.tasks.filter((t) => !t.repeatMode && !t.templateId && !t.done)
);
const oneOffDone = computed(() =>
  props.tasks.filter((t) => !t.repeatMode && !t.templateId && t.done)
);
const closedShown = computed(() =>
  showAllDone.value ? oneOffDone.value : oneOffDone.value.slice(0, DONE_PREVIEW)
);
const doneGroups = computed(() => groups.value.filter((g) => g.doneCount > 0));
const openGroups = computed(() =>
  groups.value
    .filter((g) => (g.todayInst ? !g.todayInst.done : true))
    .filter((g) => g.todayInst || g.staleIds.length || !g.doneCount || g.template)
    .sort((a, b) => {
      const rank = (g: Group) => (g.todayInst ? 0 : g.staleIds.length ? 1 : g.stopped ? 3 : 2);
      return rank(a) - rank(b);
    })
);
/** 列表头部的轻统计 */
const headerStat = computed(() => {
  const repeatOpen = groups.value.filter(
    (g) => !g.stopped && (!g.todayInst || !g.todayInst.done)
  ).length;
  return `${oneOffOpen.value.length + repeatOpen} 项进行中 · ${groups.value.length} 个重复任务`;
});

const priorityLabel = (p: number) => (p === 2 ? "高" : p === 0 ? "低" : "中");
const priorityClass = (p: number) => (p === 2 ? "high" : p === 0 ? "low" : "mid");

/** 固定事项标签 */
const repeatLabel = (mode: string): string =>
  ({ daily: "每天", weekly: "每周", monthly: "每月", yearly: "每年" })[mode] ?? "";

const dueLabel = (t: Task): string => {
  if (!t.dueTs) return "";
  const d = new Date(t.dueTs * 1000);
  const hhmm = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  const today = new Date().toDateString() === d.toDateString();
  return `${today ? "今天" : `${d.getMonth() + 1}/${d.getDate()}`} ${hhmm}`;
};

/* ---------- 次级表单：新建 / 编辑都在这里，列表只负责看 ---------- */
type FormView =
  | { mode: "new" }
  | { mode: "edit-oneoff"; task: Task }
  | { mode: "edit-repeat"; groupKey: string }
  | { mode: "edit-orphan"; groupKey: string };
const form = ref<FormView | null>(null);
const fdContent = ref("");
const fdPriority = ref(1);
const fdTime = ref<string | null>(null);
const fdRepeat = ref("");
const fdStyle = ref("");
const fdInterval = ref(0);
const fdApps = ref<string[]>([]);
const fdAppInput = ref("");
const fdSuggests = ref<string[]>([]);
const formErr = ref("");

const STYLE_OPTS = [
  { label: "默认卡片", value: "" },
  { label: "卡片", value: "card" },
  { label: "全屏", value: "fullscreen" },
  { label: "关闭", value: "none" },
];
const INTERVAL_OPTS = [
  { label: "不间隔提醒", value: 0 },
  { label: "每 5 分钟确认", value: 5 },
  { label: "每 15 分钟确认", value: 15 },
  { label: "每 30 分钟确认", value: 30 },
  { label: "每 60 分钟确认", value: 60 },
];
const REPEAT_OPTS = [
  { label: "不重复", value: "" },
  { label: "每天", value: "daily" },
  { label: "每周", value: "weekly" },
  { label: "每月", value: "monthly" },
  { label: "每年", value: "yearly" },
];
/** 正在编辑的重复任务组：从最新列表实时取（停止/清理后统计即时刷新） */
const editGroup = computed<Group | null>(() => {
  const f = form.value;
  if (!f || (f.mode !== "edit-repeat" && f.mode !== "edit-orphan")) return null;
  return groups.value.find((g) => g.key === f.groupKey) ?? null;
});

const editingTask = computed<Task | null>(() =>
  form.value?.mode === "edit-oneoff" ? form.value.task : null
);

/** 新建草稿：切走/意外退出不丢（保存成功后清除） */
const draftKey = "task.draft.new";
function loadDraft() {
  try {
    const saved = localStorage.getItem(draftKey);
    if (!saved) return;
    const d = JSON.parse(saved) as { content?: string; time?: string | null; priority?: number; repeat?: string };
    if (typeof d.content === "string" && d.content) fdContent.value = d.content;
    if (typeof d.time === "string") fdTime.value = d.time;
    if (typeof d.priority === "number") fdPriority.value = d.priority;
    if (typeof d.repeat === "string") fdRepeat.value = d.repeat;
  } catch {
    /* 忽略 */
  }
}
function saveDraft() {
  try {
    if (!fdContent.value.trim()) return;
    localStorage.setItem(
      draftKey,
      JSON.stringify({ content: fdContent.value, time: fdTime.value, priority: fdPriority.value, repeat: fdRepeat.value })
    );
  } catch {
    /* 忽略 */
  }
}
function clearDraft() {
  try {
    localStorage.removeItem(draftKey);
  } catch {
    /* 忽略 */
  }
}

function openNew() {
  form.value = { mode: "new" };
  formErr.value = "";
  fdContent.value = "";
  fdPriority.value = 1;
  fdTime.value = null;
  fdRepeat.value = "";
  loadDraft();
  void nextTick(() => listEl.value?.querySelector<HTMLInputElement>(".fd-content")?.focus());
}

function openEditTask(t: Task) {
  formErr.value = "";
  fdContent.value = t.content;
  fdPriority.value = t.priority;
  fdStyle.value = t.remindStyle ?? "";
  fdInterval.value = t.remindIntervalMin ?? 0;
  fdApps.value = [...(t.relatedApps ?? [])];
  fdAppInput.value = "";
  fdSuggests.value = [];
  if (t.repeatMode && !t.templateId) {
    form.value = { mode: "edit-repeat", groupKey: `t${t.id}` };
  } else {
    form.value = { mode: "edit-oneoff", task: t };
  }
  if (t.startTs) {
    invoke<[string, number][]>("task_suggest_apps", { id: t.id })
      .then((r) => {
        fdSuggests.value = r
          .map((x) => x[0])
          .filter((x) => !fdApps.value.includes(x))
          .slice(0, 5);
      })
      .catch(() => {});
  }
}

function openGroupEdit(g: Group) {
  if (g.template) {
    openEditTask(g.template);
  } else if (g.todayInst) {
    openEditTask(g.todayInst);
  } else {
    form.value = { mode: "edit-orphan", groupKey: g.key };
  }
}

function closeForm() {
  form.value = null;
  formErr.value = "";
}

function removeApp(a: string) {
  fdApps.value = fdApps.value.filter((x) => x !== a);
}
function addApp(a: string) {
  const v = a.trim().toLowerCase();
  if (v && !fdApps.value.includes(v)) fdApps.value.push(v);
  fdSuggests.value = fdSuggests.value.filter((x) => x !== v);
  fdAppInput.value = "";
}

async function submitNew() {
  formErr.value = "";
  if (!fdContent.value.trim()) {
    formErr.value = "先写点任务内容";
    return;
  }
  const dueTs = fdTime.value
    ? Math.floor(new Date(`${new Date().toDateString()} ${fdTime.value}`).getTime() / 1000)
    : null;
  try {
    await invoke("task_add", {
      content: fdContent.value,
      priority: fdPriority.value,
      repeatMode: fdRepeat.value,
      dueTs,
    });
    clearDraft();
    closeForm();
    emit("reload");
  } catch (e) {
    formErr.value = String(e).replace(/^.*Error: /, "");
  }
}

async function saveEdit() {
  const t = editingTask.value ?? editGroup.value?.template ?? editGroup.value?.todayInst;
  if (!t) return;
  const draft = fdContent.value.trim();
  if (!draft) {
    formErr.value = "内容不能为空";
    return;
  }
  try {
    await invoke("task_update", {
      id: t.id,
      content: draft,
      priority: fdPriority.value,
      dueTs: t.dueTs,
      remindStyle: fdStyle.value,
      remindIntervalMin: fdInterval.value,
    });
    await invoke("task_set_related_apps", { id: t.id, apps: fdApps.value });
    closeForm();
    emit("reload");
  } catch (e) {
    formErr.value = String(e).replace(/^.*Error: /, "");
  }
}

/* ---------- 列表行动作 ---------- */
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

async function toggleStart(t: Task) {
  await invoke("task_set_started", { id: t.id, ts: t.startTs ? null : Math.floor(Date.now() / 1000) });
  emit("reload");
}

async function remove(t: Task) {
  await invoke("task_delete", { id: t.id });
  emit("reload");
}

/** 停止/恢复重复任务：只影响以后生成，历史记录保留（表单里用） */
async function toggleStop(g: Group) {
  if (!g.template) return;
  await invoke("task_stop_repeat", { id: g.template.id, stopped: !g.template.repeatStopped });
  emit("reload");
}

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

/* ---------- Markdown 导入导出（低频，收在列表头部小按钮） ---------- */
async function exportMd() {
  formErr.value = "";
  try {
    const picked = await save({
      title: "导出待办为 Markdown",
      defaultPath: `拾刻待办-${new Date().toISOString().slice(0, 10)}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!picked) return;
    await invoke<string>("tasks_export_md", { path: picked });
  } catch (e) {
    formErr.value = String(e).replace(/^.*Error: /, "");
  }
}

async function importMd() {
  formErr.value = "";
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择 Markdown 文件",
      filters: [{ name: "Markdown", extensions: ["md", "markdown", "txt"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    await invoke<{ skipped: number }>("tasks_import_md", { path: picked });
    emit("reload");
  } catch (e) {
    formErr.value = String(e).replace(/^.*Error: /, "");
  }
}
</script>

<template>
  <div ref="listEl">
    <!-- ===== 列表视图 ===== -->
    <template v-if="!form">
      <div class="listhead">
        <span class="lstat">{{ headerStat }}</span>
        <div class="lacts">
          <NButton size="tiny" quaternary title="从 Markdown 导入任务" @click="importMd">导入 MD</NButton>
          <NButton size="tiny" quaternary title="导出为 Markdown 清单" @click="exportMd">导出 MD</NButton>
          <button class="newbtn" @click="openNew">
            <svg viewBox="0 0 12 12" aria-hidden="true">
              <path d="M6 2v8M2 6h8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
            </svg>
            新建任务
          </button>
        </div>
      </div>

      <!-- 一次性任务（未完成） -->
      <div v-for="t in oneOffOpen" :key="t.id" class="row">
        <NCheckbox size="small" :checked="t.done" @update:checked="(v: boolean) => toggle(t, v)" />
        <span class="prio" :class="priorityClass(t.priority)">{{ priorityLabel(t.priority) }}</span>
        <span class="content clickable" title="点击跳转详情" @click="emit('jump', t.templateId ?? t.id)">
          {{ t.content }}
        </span>
        <span v-if="t.dueTs" class="due">{{ dueLabel(t) }}</span>
        <button class="mini" :class="{ on: !!t.startTs }" :title="t.startTs ? '结束计时' : '开始做'" @click="toggleStart(t)">
          {{ t.startTs ? "计时中" : "开始" }}
        </button>
        <button class="mini edit" title="编辑" @click="openEditTask(t)">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 20h4L19 9l-4-4L4 16v4zM13 6l4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>编辑</span>
        </button>
        <NPopconfirm @positive-click="remove(t)">
          <template #trigger>
            <NButton quaternary size="tiny" type="error">删除</NButton>
          </template>
          删除这条任务？（已完成记录保留）
        </NPopconfirm>
      </div>

      <!-- 重复任务：一条一行，详情进编辑表单看 -->
      <div
        v-for="g in openGroups"
        :key="g.key"
        class="row"
        :class="{ dim: !g.todayInst || g.todayInst.done }"
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
        <span class="content clickable" title="点击跳转详情" @click="emit('jump', g.jumpId)">
          {{ g.content }}
        </span>
        <span v-if="g.todayInst?.startTs && !g.todayInst?.done" class="mini on plain">计时中</span>
        <span v-if="g.todayInst" class="due">{{ dueLabel(g.todayInst) }}</span>
        <span v-else-if="!g.stopped" class="due muted">今天无安排</span>
        <span v-if="g.repeatMode" class="rep" :title="g.stopped ? '已停止生成以后的任务' : '重复任务'">
          {{ repeatLabel(g.repeatMode) }}{{ g.stopped ? " · 已停止" : "" }}
        </span>
        <span v-else-if="g.stopped" class="rep no">{{ g.stopLabel }}</span>
        <button
          v-if="g.todayInst && !g.todayInst.done"
          class="mini"
          :class="{ on: !!g.todayInst.startTs }"
          :title="g.todayInst.startTs ? '结束计时' : '开始做今天这一次'"
          @click="toggleStart(g.todayInst)"
        >
          {{ g.todayInst.startTs ? "计时中" : "开始" }}
        </button>
        <button v-if="g.template || g.todayInst" class="mini edit" title="编辑 / 详情" @click="openGroupEdit(g)">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 20h4L19 9l-4-4L4 16v4zM13 6l4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>编辑</span>
        </button>
        <NPopconfirm v-if="g.template" @positive-click="remove(g.template!)">
          <template #trigger>
            <NButton quaternary size="tiny" type="error">删除</NButton>
          </template>
          停止生成以后的实例并隐藏这条重复任务？已完成的历史记录会保留在统计与习惯追踪里。
        </NPopconfirm>
      </div>
      <p v-if="!oneOffOpen.length && !openGroups.length" class="empty">没有进行中的任务，点右上角「新建任务」吧</p>

      <p v-if="oneOffDone.length" class="done-head">
        已完成 · {{ oneOffDone.length }}（显示{{ showAllDone ? "全部" : `最近 ${DONE_PREVIEW} 条` }})
        <button v-if="oneOffDone.length > DONE_PREVIEW" class="mini" @click="showAllDone = !showAllDone">
          {{ showAllDone ? "收起" : "展开全部" }}
        </button>
      </p>
      <div v-for="t in closedShown" :key="'d' + t.id" class="row done">
        <NCheckbox size="small" :checked="t.done" @update:checked="(v: boolean) => toggle(t, v)" />
        <span class="content strikethrough">{{ t.content }}</span>
        <span v-if="t.dueTs" class="due">{{ dueLabel(t) }}</span>
        <NPopconfirm @positive-click="remove(t)">
          <template #trigger>
            <NButton quaternary size="tiny" type="error">删除</NButton>
          </template>
          删除这条任务？
        </NPopconfirm>
      </div>

      <!-- 重复任务历史：合并计数（逐天在习惯追踪 / 详情页看） -->
      <template v-if="doneGroups.length">
        <p class="done-head">重复任务历史</p>
        <div v-for="g in doneGroups" :key="'g' + g.key" class="row done">
          <span class="content strikethrough clickable" title="点击跳转详情看每天情况" @click="emit('jump', g.jumpId)">
            {{ g.content }}
          </span>
          <span v-if="g.repeatMode" class="rep">{{ repeatLabel(g.repeatMode) }}{{ g.stopped ? " · 已停止" : "" }}</span>
          <span v-else class="rep no">{{ g.stopLabel }}</span>
          <span class="hcount">已完成 {{ g.doneCount }} 次</span>
          <button class="mini edit" title="坚持详情" @click="openGroupEdit(g)">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 20h4L19 9l-4-4L4 16v4zM13 6l4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>详情</span>
        </button>
        </div>
      </template>
    </template>

    <!-- ===== 次级表单：新建 / 编辑 ===== -->
    <template v-else>
      <div class="formhead">
        <button class="mini back" @click="closeForm">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M14.5 5.5L8 12l6.5 6.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          返回列表
        </button>
        <span class="ftitle">{{ form.mode === "new" ? "新建任务" : "编辑任务" }}</span>
      </div>

      <!-- 新建 -->
      <template v-if="form.mode === 'new'">
        <input
          v-model="fdContent"
          class="finput fd-content"
          placeholder="任务内容…（时间留空 = 今天）"
          maxlength="200"
          @keyup.enter="submitNew"
          @input="saveDraft"
        />
        <div class="frow">
          <select v-model.number="fdPriority" class="esel">
            <option :value="2">高优先</option>
            <option :value="1">中优先</option>
            <option :value="0">低优先</option>
          </select>
          <input v-model="fdTime" type="time" class="esel tp" />
          <select v-model="fdRepeat" class="esel">
            <option v-for="o in REPEAT_OPTS" :key="o.value" :value="o.value">{{ o.label }}</option>
          </select>
          <button class="primary" @click="submitNew">添加</button>
        </div>
        <p class="fnote">选了「每天 / 每周…」就是固定事项：每天只占任务列表一行，完成情况进习惯追踪；随时可以停止重复，历史记录不会丢。</p>
      </template>

      <!-- 编辑一次性任务 / 模板 / 孤儿组 -->
      <template v-else>
        <template v-if="editingTask || editGroup?.template || editGroup?.todayInst">
          <input v-model="fdContent" class="finput" maxlength="200" @keyup.enter="saveEdit" />
          <div class="frow">
            <select v-model.number="fdPriority" class="esel">
              <option :value="2">高优先</option>
              <option :value="1">中优先</option>
              <option :value="0">低优先</option>
            </select>
            <select v-model="fdStyle" class="esel" title="到期提醒方式">
              <option v-for="o in STYLE_OPTS" :key="o.value" :value="o.value">到期提醒：{{ o.label }}</option>
            </select>
            <select v-model="fdInterval" class="esel" title="计时中的间隔确认">
              <option v-for="o in INTERVAL_OPTS" :key="o.value" :value="o.value">{{ o.label }}</option>
            </select>
            <button class="primary" @click="saveEdit">保存</button>
          </div>
          <div class="frow appsrow">
            <span class="flabel" title="用于把心流时段自动归属到这个任务">相关应用</span>
            <span v-for="a in fdApps" :key="a" class="appchip" title="点击移除" @click="removeApp(a)">
              {{ a.replace(".exe", "") }} ✕
            </span>
            <button v-for="a in fdSuggests" :key="'s' + a" class="appchip sug" title="追踪期高频应用，点击添加" @click="addApp(a)">
              + {{ a.replace(".exe", "") }}
            </button>
            <input v-model="fdAppInput" class="eapp" placeholder="手动加应用名" @keyup.enter="addApp(fdAppInput)" />
          </div>
          <p v-if="form.mode === 'edit-repeat'" class="fnote">
            这里编辑的是重复任务本身：内容 / 优先级 / 提醒设置会同步到以后未完成的每一天，已完成的历史不变。
          </p>
        </template>

        <!-- 重复任务专属：统计与停止/清理 -->
        <template v-if="editGroup">
          <div class="frow">
            <span class="flabel">坚持情况</span>
            <span class="hcount">已完成 {{ editGroup.doneCount }} 次</span>
            <span v-if="editGroup.staleIds.length" class="hcount warn">{{ editGroup.staleIds.length }} 天未做</span>
          </div>
          <div v-if="editGroup.template" class="frow">
            <button class="mini stop" @click="toggleStop(editGroup)">
              {{ editGroup.template.repeatStopped ? "恢复重复" : "停止重复" }}
            </button>
            <span class="fnote inline">
              {{ editGroup.template.repeatStopped ? "恢复后会继续按「" + repeatLabel(editGroup.repeatMode) + "」生成" : "停止后不再生成以后的实例，历史记录全部保留" }}
            </span>
          </div>
          <div v-if="editGroup.staleIds.length" class="frow">
            <NPopconfirm @positive-click="clearStale(editGroup)">
              <template #trigger>
                <button class="mini stop" :disabled="clearing">清理 {{ editGroup.staleIds.length }} 天过期</button>
              </template>
              把过期未做的旧实例删掉？（已完成的历史记录不受影响）
            </NPopconfirm>
          </div>
        </template>
      </template>

      <p v-if="formErr" class="ferr">{{ formErr }}</p>
    </template>
  </div>
</template>

<style scoped>
.listhead {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 10px;
}

.lstat {
  font-size: 12px;
  color: var(--text-faint);
}

.lacts {
  display: flex;
  align-items: center;
  gap: 4px;
}

/* 新建任务胶囊（与月历定位按钮同族） */
.newbtn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border: 1px solid var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent-text);
  border-radius: var(--r-full);
  font-size: 12px;
  font-family: inherit;
  padding: 4px 12px;
  cursor: pointer;
  transition: background var(--dur), color var(--dur);
}

.newbtn svg {
  width: 11px;
  height: 11px;
}

.newbtn:hover {
  background: var(--accent-text);
  color: var(--surface);
}

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

.due {
  flex: none;
  font-size: 11px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.due.muted {
  color: var(--text-faint);
}

.rep {
  font-size: 10px;
  color: var(--accent-text);
  background: var(--accent-soft);
  border-radius: var(--r-full);
  padding: 1px 8px;
  flex: none;
}

.rep.no {
  color: var(--text-faint);
  background: var(--surface);
}

.hcount {
  font-size: 11px;
  color: var(--text-faint);
  flex: none;
}

.hcount.warn {
  color: var(--warn);
}

/* 行内小按钮：开始 / 编辑 / 展开等 */
.mini {
  flex: none;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-faint);
  border-radius: var(--r-full);
  font-size: 10px;
  font-family: inherit;
  padding: 2px 9px;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  transition: color var(--dur), border-color var(--dur), background var(--dur);
}

/* 编辑/详情胶囊：与日志卡同款——默认圆形图标，悬停展开文字 */
.mini.edit {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 24px;
  padding: 0;
  color: var(--accent-text);
  border-color: var(--accent-border);
  overflow: hidden;
  transition: width var(--dur) cubic-bezier(0.22, 0.61, 0.36, 1), background var(--dur), color var(--dur), border-color var(--dur);
}

.mini.edit:hover {
  width: 64px;
  background: var(--accent-soft);
}

.mini.edit span {
  max-width: 0;
  overflow: hidden;
  font-size: 11px;
  white-space: nowrap;
  opacity: 0;
  transition: max-width var(--dur) ease, opacity var(--dur) ease, margin-left var(--dur) ease;
}

.mini.edit:hover span {
  max-width: 34px;
  opacity: 1;
  margin-left: 4px;
}

.mini.on {
  color: var(--good);
  border-color: var(--good);
  background: var(--good-soft);
}

.mini.on.plain {
  border-style: dashed;
  cursor: default;
}

.mini.stop {
  color: var(--warn);
  border-color: var(--warn);
}

.mini:disabled {
  opacity: 0.5;
  cursor: default;
}

.mini svg {
  width: 11px;
  height: 11px;
}

.empty,
.done-head {
  font-size: 12px;
  color: var(--text-faint);
  margin: 6px 0;
}

/* ---- 次级表单 ---- */
.formhead {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
}

.mini.back {
  font-size: 11px;
  padding: 4px 10px;
  color: var(--text-muted);
}

.mini.back:hover {
  color: var(--accent-text);
  border-color: var(--accent-border);
  background: var(--accent-soft);
}

.ftitle {
  font-size: 14px;
  font-weight: 700;
  color: var(--text);
}

.finput {
  width: 100%;
  border: 1px solid var(--accent-border);
  background: var(--surface);
  color: var(--text);
  border-radius: 8px;
  padding: 8px 12px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
  margin-bottom: 10px;
}

.finput:focus {
  border-color: var(--accent-text);
}

.frow {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 10px;
}

.frow.appsrow {
  margin-bottom: 6px;
}

.flabel {
  flex: none;
  font-size: 11px;
  color: var(--text-muted, #888);
}

.esel {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  border-radius: 8px;
  font-size: 12px;
  font-family: inherit;
  padding: 6px 8px;
  outline: none;
}

.esel.tp {
  padding: 5px 8px;
}

.primary {
  border: 1px solid var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent-text);
  border-radius: var(--r-full);
  font-size: 12px;
  font-family: inherit;
  padding: 5px 16px;
  cursor: pointer;
  transition: background var(--dur), color var(--dur);
}

.primary:hover {
  background: var(--accent-text);
  color: var(--surface);
}

.fnote {
  margin: 0;
  font-size: 11px;
  color: var(--text-faint);
  line-height: 1.6;
}

.fnote.inline {
  margin: 0;
}

.ferr {
  margin: 0;
  font-size: 12px;
  color: var(--danger);
}

/* 相关应用 chips */
.appchip {
  font-size: 11px;
  padding: 2px 9px;
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
  padding: 3px 7px;
}
</style>
