<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NInput, NSelect } from "naive-ui";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { ReminderRule, Task, TodoStats } from "../types";
import Icon from "../components/Icon.vue";
import TaskBoard from "../components/TaskBoard.vue";
import ReminderRules from "../components/ReminderRules.vue";
import CalendarHeat from "../components/CalendarHeat.vue";
import NotesPanel from "../components/NotesPanel.vue";
import DayDetail from "../components/DayDetail.vue";
import { jumpTo, navIntent, notesIntent, tabIntent, activeTab } from "../lib/uiState";
import HabitGrid from "../components/HabitGrid.vue";
import type { HabitRow } from "../types";

const tasks = ref<Task[]>([]);
const rules = ref<ReminderRule[]>([]);
const stats = ref<TodoStats | null>(null);

const newContent = ref("");
const newPriority = ref(1);
const newDue = ref<string | null>(null);
const err = ref("");
const msg = ref("");
/** 月历选中的那天，传给日志面板 */
const pickedDay = ref<number>(Date.now());

const repeatOptions = [
  { label: "不重复", value: "" },
  { label: "每天", value: "daily" },
  { label: "每周", value: "weekly" },
  { label: "每月", value: "monthly" },
  { label: "每年", value: "yearly" },
];
const newRepeat = ref("");

/* 正在输入的任务本地草稿：切走/意外退出不丢（保存成功后清除） */
const taskDraftKey = "task.draft.new";
try {
  const saved = localStorage.getItem(taskDraftKey);
  if (saved) {
    const d = JSON.parse(saved) as { content?: string; due?: string | null; priority?: number; repeat?: string };
    if (typeof d.content === "string" && d.content) {
      newContent.value = d.content;
      if (typeof d.due === "string") newDue.value = d.due;
      if (typeof d.priority === "number") newPriority.value = d.priority;
      if (typeof d.repeat === "string") newRepeat.value = d.repeat;
    }
  }
} catch {
  /* 忽略 */
}
let taskDraftTimer = 0;
function saveTaskDraft() {
  try {
    if (!newContent.value.trim()) return;
    localStorage.setItem(
      taskDraftKey,
      JSON.stringify({ content: newContent.value, due: newDue.value, priority: newPriority.value, repeat: newRepeat.value })
    );
  } catch {
    /* 忽略 */
  }
}
watch(newContent, () => {
  window.clearTimeout(taskDraftTimer);
  taskDraftTimer = window.setTimeout(saveTaskDraft, 400);
});
watch([newDue, newPriority, newRepeat], () => saveTaskDraft());

const debouncedHabitRefresh = ref(0);
let habitRefreshTimer = 0;
watch(() => tasks.value.map((t) => `${t.id}:${t.done ? 1 : 0}`).join(","), () => {
  window.clearTimeout(habitRefreshTimer);
  habitRefreshTimer = window.setTimeout(() => (debouncedHabitRefresh.value = Date.now()), 600);
});

const priorityOptions = [
  { label: "高", value: 2 },
  { label: "中", value: 1 },
  { label: "低", value: 0 },
];

/** Markdown 导出（对齐 Obsidian / Notion 的 - [ ] 语法） */
async function exportMd() {
  err.value = "";
  try {
    const picked = await save({
      title: "导出待办为 Markdown",
      defaultPath: `拾刻待办-${new Date().toISOString().slice(0, 10)}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!picked) return;
    const file = await invoke<string>("tasks_export_md", { path: picked });
    msg.value = `已导出：${file}`;
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

/** Markdown 导入（同内容的任务自动跳过） */
async function importMd() {
  err.value = "";
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择 Markdown 文件",
      filters: [{ name: "Markdown", extensions: ["md", "markdown", "txt"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    const sum = await invoke<{ skipped: number }>("tasks_import_md", { path: picked });
    await load();
    msg.value = `导入完成，跳过 ${sum.skipped} 条已存在的任务`;
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

/** 提醒规则：结构化字段多，用 JSON 才能完整回环 */
async function exportRules() {
  err.value = "";
  try {
    const picked = await save({
      title: "导出提醒规则",
      defaultPath: `拾刻提醒规则-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!picked) return;
    const file = await invoke<string>("rules_export_json", { path: picked });
    msg.value = `已导出规则：${file}`;
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function importRules() {
  err.value = "";
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择规则 JSON",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    const [added, skipped] = await invoke<[number, number]>("rules_import_json", { path: picked });
    await load();
    msg.value = `导入规则 ${added} 条，跳过同名的 ${skipped} 条`;
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

/** 整包导出：任务 MD + 规则 JSON（同一目录、同一时间戳） */
async function exportAll() {
  err.value = "";
  try {
    const picked = await save({
      title: "整包导出（任务 + 提醒规则）",
      defaultPath: `拾刻导出-${new Date().toISOString().slice(0, 10)}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!picked) return;
    const stamp = new Date().toISOString().slice(0, 10);
    const dir = String(picked).replace(/[^\/]*$/, "");
    await invoke("tasks_export_md", { path: picked });
    await invoke("rules_export_json", { path: `${dir}拾刻提醒规则-${stamp}.json` });
    msg.value = `已整包导出到：${dir}`;
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

/** 二级界面：只负责开关，内容由 DayDetail 组件自己取数 */
const detailDate = ref("");
const sheetBig = ref(false);

/* 遮罩只有"按下和松开都在遮罩上"才关：在输入框里拖选文字滑出卡片不会误关 */
let maskDownOnSelf = false;
function maskDown(e: MouseEvent) {
  maskDownOnSelf = e.target === e.currentTarget;
}
function maskRelease(e: MouseEvent, close: () => void) {
  if (maskDownOnSelf && e.target === e.currentTarget) close();
  maskDownOnSelf = false;
}

/** 日志面板当前编辑的日期（放大按钮跳「详细 · 事项」用） */
function noteDateStr(): string {
  const d = new Date(pickedDay.value);
  const m = `${d.getMonth() + 1}`.padStart(2, "0");
  const day = `${d.getDate()}`.padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

function openDetail(date: string) {
  detailDate.value = date;
}

/** 放大：不在这里放大，而是跳到 详细 · 事项 里看同一天 */
function openInDetail() {
  jumpTo("items", undefined, detailDate.value);
  detailDate.value = "";
}

/** 点日历：切到那天 + 打开二级界面 */
/* ---------- 三卡并列的二级框 ---------- */
const openModule = ref<"" | "tasks" | "rules" | "notes">("");
const noteCount = ref(0);
const lastNoteDate = ref("");

const moduleTitle = computed(
  () => ({ tasks: "任务", rules: "提示", notes: "日志" })[openModule.value as "tasks"] ?? ""
);

async function loadNoteSummary() {
  try {
    const list = await invoke<{ date: string }[]>("notes_recent", { limit: 200 });
    noteCount.value = list.length;
    lastNoteDate.value = list[0]?.date ?? "";
  } catch {
    /* 忽略 */
  }
}

function onPickDay(date: string) {
  pickedDay.value = new Date(`${date}T00:00:00`).getTime();
  void openDetail(date);
}

/** 定位按钮：只回到本月今天，不弹二级界面 */
function onLocate(date: string) {
  pickedDay.value = new Date(`${date}T00:00:00`).getTime();
}

/* ---------- 今日待办：今天到期 + 未填时间的（默认按当天算） ---------- */
const todayStr = () => new Date().toISOString().slice(0, 10);
const taskDay = (t: Task) =>
  t.dueTs ? new Date(t.dueTs * 1000).toISOString().slice(0, 10) : todayStr();

const todayTasks = computed(() =>
  tasks.value
    .filter((t) => taskDay(t) === todayStr())
    .sort((a, b) => {
      if (a.done !== b.done) return a.done ? 1 : -1;
      const at = a.dueTs ?? Number.MAX_SAFE_INTEGER;
      const bt = b.dueTs ?? Number.MAX_SAFE_INTEGER;
      if (at !== bt) return at - bt;
      return b.priority - a.priority;
    })
);

function dueLabel(t: Task): string {
  if (!t.dueTs) return "今天";
  const d = new Date(t.dueTs * 1000);
  return `${`${d.getHours()}`.padStart(2, "0")}:${`${d.getMinutes()}`.padStart(2, "0")}`;
}

async function toggleTask(t: Task) {
  try {
    await invoke("task_set_done", { id: t.id, done: !t.done });
    await load();
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}


async function load() {
  try {
    tasks.value = await invoke<Task[]>("task_list");
    rules.value = await invoke<ReminderRule[]>("rule_list");
    stats.value = await invoke<TodoStats>("todo_stats");
  } catch {
    /* 忽略 */
  }
}


/** 点任务名 → 详细 · 事项，按任务看坚持情况 */
function jumpToDetail(taskId: number) {
  jumpTo("task", undefined, undefined, taskId);
}

/* 任何跨页跳转都先关掉本页的二级框：不然遮罩盖着新页面，看起来像"点了没反应" */
watch([navIntent, tabIntent], () => {
  openModule.value = "";
  detailDate.value = "";
});

/* 详细 · 事项 里点"编辑"→ 打开日志面板并定位到那天 */
watch(notesIntent, async (n) => {
  if (!n) return;
  notesIntent.value = null;
  pickedDay.value = new Date(`${n.date}T00:00:00`).getTime();
  openModule.value = "notes";
  await load();
});

/* ---------- 习惯追踪（固定事项完成格，挪到待办页底部；点格子直接打卡） ---------- */
const habitRows = ref<HabitRow[]>([]);
const selHabitId = ref<number | null>(null);

async function loadHabit() {
  try {
    habitRows.value = await invoke<HabitRow[]>("habit_grid", { days: 30 });
  } catch {
    habitRows.value = [];
  }
}

async function toggleHabitCell(taskId: number, date: string) {
  try {
    await invoke("habit_toggle", { taskId, date });
    await loadHabit();
    await load();
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

// 切回待办页 / 任务有变动时都刷新完成格
watch(activeTab, (t) => {
  if (t === "todo") void loadHabit();
});
watch(debouncedHabitRefresh, () => void loadHabit());

async function addTask() {
  err.value = "";
  if (!newContent.value.trim()) {
    err.value = "先写点任务内容";
    return;
  }
  const dueTs = newDue.value
    ? Math.floor(new Date(`${new Date().toDateString()} ${newDue.value}`).getTime() / 1000)
    : null;
  try {
    await invoke("task_add", {
      content: newContent.value,
      priority: newPriority.value,
      repeatMode: newRepeat.value,
      dueTs,
    });
    newContent.value = "";
    newDue.value = null;
    try {
      localStorage.removeItem(taskDraftKey);
    } catch {
      /* 忽略 */
    }
    await load();
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

onMounted(async () => {
  await load();
  void loadNoteSummary();
  void loadHabit();
});
const rateLabel = (v: number) => (v < 0 ? "—" : `${v}%`);
const bucketLabels = ["<15分", "15~60分", "1~4时", "4~24时", "≥1天"];
const bucketMax = () => Math.max(1, ...(stats.value?.buckets ?? [1]));
</script>

<template>
  <div class="todo">
    <header class="head">
      <h1>待办</h1>
      <span class="sub">任务、提醒与完成率</span>
      <div class="headacts">
        <NButton size="small" secondary @click="exportAll">整包导出</NButton>
      </div>
    </header>
    <p v-if="msg" class="okline">{{ msg }}</p>

    <!-- 顶部：左月历 + 右今日待办（约 3:2，等高对齐） -->
    <section class="toprow">
      <div class="glass-card calcard">
        <h2>日程 · 月历</h2>
        <CalendarHeat
          compact
          :selected="new Date(pickedDay).toISOString().slice(0, 10)"
          @pick="onPickDay"
          @locate="onLocate"
        />
        <p class="hint">点某一天进入那天的详情；颜色越深表示那天用得越久</p>
      </div>
      <div class="glass-card todaycard">
        <div class="modhead">
          <h2>今日待办</h2>
          <span class="cnt">{{ todayTasks.length }} 项</span>
        </div>
        <div class="todaylist">
          <label v-for="t in todayTasks" :key="t.id" class="trow" :class="{ done: t.done }">
            <input type="checkbox" :checked="t.done" @change="toggleTask(t)" />
            <span class="tname">{{ t.content }}</span>
            <span class="ttime">{{ dueLabel(t) }}</span>
            <span class="tpri" :class="`p${t.priority}`">{{ ["低", "中", "高"][t.priority] ?? "中" }}</span>
          </label>
          <p v-if="!todayTasks.length" class="empty">今天没有待办，去下面加一条吧</p>
        </div>
      </div>
    </section>

    <!-- 完成率统计（已移入"点日历某天"的二级界面） -->
    <section v-if="false" class="cards">
      <div class="glass-card stat">
        <p class="label"><Icon name="checklist" :size="14" /> 今日完成</p>
        <p class="value accent">{{ stats?.todayDone ?? 0 }} <small>件</small></p>
      </div>
      <div class="glass-card stat">
        <p class="label"><Icon name="graph" :size="14" /> 本周完成率</p>
        <p class="value">{{ rateLabel(stats?.weekRate ?? -1) }}</p>
      </div>
      <div class="glass-card stat">
        <p class="label"><Icon name="target" :size="14" /> 按时完成率</p>
        <p class="value">{{ rateLabel(stats?.ontimeRate ?? -1) }}</p>
      </div>
      <div class="glass-card stat">
        <p class="label"><Icon name="clock" :size="14" /> 平均完成用时</p>
        <p class="value small">{{ stats?.avgMinutes ?? 0 }} 分钟</p>
      </div>
    </section>

    <!-- 任务看板 -->
    <!-- 三卡并列：任务 / 待办提醒 / 日志（点卡片在二级框里编辑） -->
    <section class="modrow">
      <button class="glass-card mcard" @click="openModule = 'tasks'">
        <div class="mhead">
          <h2>任务</h2>
          <Icon name="checklist" :size="16" />
        </div>
        <p class="mnum">{{ tasks.filter((t) => !t.done).length }} <small>项未完成</small></p>
        <p class="msub">今日待办 {{ todayTasks.length }} 项 · 点开编辑清单</p>
      </button>

      <button class="glass-card mcard" @click="openModule = 'rules'">
        <div class="mhead">
          <h2>提示</h2>
          <Icon name="bell" :size="16" />
        </div>
        <p class="mnum">{{ rules.filter((r) => r.enabled).length }} <small>条启用中</small></p>
        <p class="msub">共 {{ rules.length }} 条规则 · 点开管理</p>
      </button>

      <button class="glass-card mcard" @click="openModule = 'notes'">
        <div class="mhead">
          <h2>日志</h2>
          <Icon name="doc" :size="16" />
        </div>
        <p class="mnum">{{ noteCount }} <small>篇</small></p>
        <p class="msub">{{ lastNoteDate || "还没有日志" }} · 点开写今天的</p>
      </button>
    </section>


    <!-- 习惯追踪：固定事项完成格（点格子直接打卡，点行名选中看趋势） -->
    <section class="glass-card habithost">
      <HabitGrid
        :rows="habitRows"
        :model-value="selHabitId"
        @select="selHabitId = $event"
        @toggle="toggleHabitCell"
      />
    </section>

    <!-- 完成区间分布（已迁到洞察页，这里停用） -->
    <section v-if="false" class="glass-card">
      <h2>完成用时分布（全部任务）</h2>
      <div class="buckets">
        <div v-for="(b, i) in stats?.buckets ?? []" :key="i" class="bcol">
          <div class="bbar-wrap">
            <div
              class="bbar"
              :style="{ height: (b / bucketMax()) * 72 + 'px' }"
              :title="`${b} 件`"
            ></div>
          </div>
          <span class="bl">{{ bucketLabels[i] }}</span>
          <span class="bv">{{ b }}</span>
        </div>
      </div>
    </section>

    <!-- 提醒规则 -->
    <!-- 某天详情（二级视图，先搭框架）。Teleport 到 body：卡片的 backdrop-filter 会创建包含块，
         不 Teleport 的话 position:fixed 会被困在卡片内部（表现成"同级卡片"） -->
    <!-- 模块二级框：任务 / 待办提醒 / 日志 -->
    <Teleport to="body">
      <div v-show="openModule" class="mask" @mousedown="maskDown" @mouseup="maskRelease($event, () => (openModule = ''))">
        <span class="grain" aria-hidden="true"></span>
        <div class="sheet glass-card">
          <div class="modhead">
            <h2>{{ moduleTitle }}</h2>
            <div class="dacts">
              <!-- 日志二级框：放大图标与关闭并排，跳「详细 · 事项」看同一天 -->
              <button
                v-if="openModule === 'notes'"
                class="wbtn"
                title="在「详细 · 事项」中打开"
                @click="jumpTo('items', undefined, noteDateStr())"
              >
                <svg width="12" height="12" viewBox="0 0 12 12">
                  <rect x="2.5" y="2.5" width="7" height="7" rx="1" fill="none" stroke="currentColor" stroke-width="1.2" />
                </svg>
              </button>
              <button class="wbtn" title="关闭" @click="openModule = ''">
                <svg width="12" height="12" viewBox="0 0 12 12">
                  <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" />
                </svg>
              </button>
            </div>
          </div>
          <template v-if="openModule === 'tasks'">
            <div class="addrow">
              <NInput
                v-model:value="newContent"
                size="small"
                placeholder="添加任务…（时间留空 = 今天）"
                @keyup.enter="addTask"
              />
              <NSelect v-model:value="newPriority" size="small" :options="priorityOptions" style="width: 84px" />
              <input v-model="newDue" type="time" class="tp" />
              <NSelect v-model:value="newRepeat" size="small" :options="repeatOptions" style="width: 92px" />
              <NButton size="small" type="primary" secondary @click="addTask">添加</NButton>
              <NButton size="tiny" quaternary @click="importMd">导入 MD</NButton>
              <NButton size="tiny" quaternary @click="exportMd">导出 MD</NButton>
            </div>
            <TaskBoard :tasks="tasks" @reload="load" @jump="jumpToDetail" />
          </template>
          <template v-else-if="openModule === 'rules'">
            <div class="addrow">
              <NButton size="tiny" quaternary @click="importRules">导入 JSON</NButton>
              <NButton size="tiny" quaternary @click="exportRules">导出 JSON</NButton>
              <span class="msub">规则支持卡片 / 全屏两种提醒方式</span>
            </div>
            <ReminderRules :rules="rules" @reload="load" />
          </template>
          <NotesPanel v-else-if="openModule === 'notes'" :day-ts="pickedDay" />
        </div>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-show="detailDate" class="mask" @mousedown="maskDown" @mouseup="maskRelease($event, () => (detailDate = ''))">
        <span class="grain" aria-hidden="true"></span>
      <div class="sheet glass-card" :class="{ big: sheetBig }">
        <div class="modhead">
          <h2>{{ detailDate }} · 那天</h2>
          <div class="dacts">
            <!-- 与窗口控制同款图标：□ 放大（跳转到 详细 · 事项）、× 关闭 -->
            <button class="wbtn" title="在「详细 · 事项」中打开" @click="openInDetail">
              <svg width="12" height="12" viewBox="0 0 12 12">
                <rect x="2.5" y="2.5" width="7" height="7" rx="1" fill="none" stroke="currentColor" stroke-width="1.2" />
              </svg>
            </button>
            <button class="wbtn" title="关闭" @click="detailDate = ''">
              <svg width="12" height="12" viewBox="0 0 12 12">
                <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" />
              </svg>
            </button>
          </div>
        </div>
        <DayDetail :date="detailDate" />
      </div>
      </div>
    </Teleport>

    <!-- 每日日志 -->
  </div>
</template>

<style scoped>
.todo {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

/* 弹层里的操作行（新建任务 / 导入导出） */
.addrow {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  margin-bottom: 10px;
}

.tp {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  border-radius: var(--r-sm);
  font-size: 12px;
  font-family: inherit;
  padding: 4px 8px;
}

/* 三卡并列：等宽、等高、可点 */
.modrow {
  display: flex;
  gap: 14px;
  align-items: stretch;
}

.mcard {
  flex: 1 1 0;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
  text-align: left;
  padding: 14px 16px;
  border: 1px solid var(--card-border);
  cursor: pointer;
  font-family: inherit;
}

.mhead {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--text-muted);
}

.mhead h2 {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

.mnum {
  margin: 0;
  font-size: 22px;
  font-weight: 700;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.mnum small {
  font-size: 12px;
  font-weight: 400;
  color: var(--text-faint);
}

.msub {
  margin: 0;
  font-size: 11.5px;
  color: var(--text-faint);
}

@media (max-width: 900px) {
  .modrow {
    flex-direction: column;
  }
}

.hint {
  margin: 8px 0 0;
  font-size: 11px;
  color: var(--text-faint);
}

/* 顶部两列：月历 3 : 今日待办 2，等高对齐 */
.toprow {
  display: flex;
  gap: 14px;
  align-items: stretch;
}

/* 月历 3 : 今日待办 2，两卡等高 */
.calcard {
  flex: 3 1 0;
  min-width: 0;
}

.todaycard {
  flex: 2 1 0;
  min-width: 0;
}

/* 窗口太窄时改为上下堆叠，避免挤成一团 */
@media (max-width: 900px) {
  .toprow {
    flex-direction: column;
  }
}

.calcard,
.todaycard {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.cnt {
  font-size: 11px;
  color: var(--text-faint);
}

.todaylist {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding-right: 4px;
}

.trow {
  display: grid;
  grid-template-columns: 16px 1fr auto 22px;
  align-items: center;
  gap: 8px;
  border: 1px solid transparent;
  background: var(--surface);
  border-radius: var(--r-sm);
  padding: 6px 9px;
  font-size: 12px;
  cursor: pointer;
  transition: background var(--dur), transform var(--dur), border-color var(--dur);
}

.trow:hover {
  background: var(--surface-hover);
  transform: translateX(calc(2px * var(--motion)));
}

.trow.done .tname {
  text-decoration: line-through;
  color: var(--text-faint);
}

.tname {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}

.ttime {
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

.tpri {
  font-size: 10px;
  text-align: center;
  border-radius: var(--r-full);
  padding: 1px 0;
}

.tpri.p2 {
  color: var(--danger);
  background: var(--danger-soft);
}

.tpri.p1 {
  color: var(--info);
  background: var(--info-soft);
}

.tpri.p0 {
  color: var(--text-faint);
  background: var(--surface-hover);
}

/* 二级视图（某天详情） */
.mask {
  position: fixed;
  /* 覆盖整窗（含外壳那 16px 留白），否则页面滚动条会露在未磨砂区域 */
  inset: 0;
  z-index: 40;
  /* 遮罩用主题底色而不是纯黑：浅色模式下不再"压黑" */
  /* 磨砂玻璃：更重的模糊 + 轻微降亮 + 一层细颗粒（否则只是"糊"，没有质感） */
  background: rgba(var(--bg-rgb), 0.3);
  backdrop-filter: blur(24px) saturate(1.12) brightness(0.97);
  -webkit-backdrop-filter: blur(24px) saturate(1.12) brightness(0.97);
  border-radius: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  /* 四周留白：窗口模式下弹层不再顶住上下边框 */
  padding: 40px 32px;
}

/* 细颗粒层：给磨砂玻璃一点"砂"的质感 */
.grain {
  position: absolute;
  inset: 0;
  pointer-events: none;
  opacity: 0.4;
  border-radius: 0;
  background-image: url("data:image/svg+xml;utf8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='120' height='120'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='2'/%3E%3C/filter%3E%3Crect width='120' height='120' filter='url(%23n)' opacity='0.5'/%3E%3C/svg%3E");
}

.dacts {
  display: flex;
  align-items: center;
  gap: 2px;
}

.wbtn {
  width: 28px;
  height: 26px;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  border-radius: var(--r-sm);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: background var(--dur), color var(--dur);
}

.dbtn:hover {
  background: var(--surface-hover);
  color: var(--text);
}

.sheet {
  width: min(700px, 100%);
  max-height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 18px 20px;
  transition: width var(--dur) ease, height var(--dur) ease;
  /* 弹层本身就用卡片那套材质与配色 */
  background: var(--bg-glass);
}

/* 放大：铺满可用区域（类似 Notion 的展开查看） */
.sheet.big {
  width: 100%;
  height: 100%;
  max-height: 100%;
}

.dcards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 10px;
}

.dstat {
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-md);
  padding: 10px 12px;
}

.dl {
  margin: 0 0 4px;
  font-size: 11px;
  color: var(--text-muted);
}

.dv {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.dsec {
  border-top: 1px solid var(--border);
  padding-top: 10px;
}

.dimgs {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.dimgs img {
  width: 108px;
  height: 80px;
  object-fit: cover;
  border-radius: var(--r-sm);
  border: 1px solid var(--border);
  cursor: zoom-in;
}

.big {
  max-width: min(900px, 92%);
  max-height: 92%;
  border-radius: var(--r-md);
  box-shadow: var(--card-shadow-hover);
}

.dtext {
  margin: 0;
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.7;
}

.modhead {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.modacts {
  display: flex;
  gap: 4px;
}

.headacts {
  margin-left: auto;
  display: flex;
  gap: 8px;
}

.okline {
  margin: 0;
  font-size: 12px;
  color: var(--good);
  word-break: break-all;
}

.head {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.head h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
}

.sub {
  font-size: 12px;
  color: var(--text-muted);
}

.cards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
}

.glass-card {
  padding: 16px 18px;
}

.glass-card h2 {
  margin: 0 0 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

.stat .label {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 6px;
}

.stat .value {
  margin: 0;
  font-size: 22px;
  font-weight: 700;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.stat .value.accent {
  color: var(--accent-text);
}

.stat .value.small {
  font-size: 16px;
}

.stat small {
  font-size: 12px;
  font-weight: 400;
  color: var(--text-faint);
}

.add {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}

.add > :first-child {
  flex: 1;
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

.err {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--danger);
}

.buckets {
  display: flex;
  gap: 22px;
  align-items: flex-end;
  padding: 4px 8px 0;
}

.bcol {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.bbar-wrap {
  height: 74px;
  display: flex;
  align-items: flex-end;
}

.bbar {
  width: 34px;
  background: linear-gradient(180deg, var(--accent), var(--accent-soft));
  border-radius: 6px 6px 2px 2px;
  transition: height 0.3s;
}

.bl {
  font-size: 10px;
  color: var(--text-faint);
}

.bv {
  font-size: 12px;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}
</style>
