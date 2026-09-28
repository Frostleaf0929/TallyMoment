<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
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

function openDetail(date: string) {
  detailDate.value = date;
}

/** 点日历：切到那天 + 打开二级界面 */
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
      dueTs,
    });
    newContent.value = "";
    newDue.value = null;
    await load();
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

onMounted(load);
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
    <section class="glass-card">
      <div class="modhead">
        <h2>任务</h2>
        <div class="modacts">
          <NButton size="tiny" quaternary @click="importMd">导入 MD</NButton>
          <NButton size="tiny" quaternary @click="exportMd">导出 MD</NButton>
        </div>
      </div>
      <div class="add">
        <NInput
          v-model:value="newContent"
          size="small"
          placeholder="添加任务，双击已有任务可改内容"
          maxlength="200"
          @keyup.enter="addTask"
        />
        <NSelect
          v-model:value="newPriority"
          size="small"
          :options="priorityOptions"
          :show-arrow="false"
          style="width: 76px"
        />
        <input v-model="newDue" type="time" class="tp" />
        <NButton size="small" type="primary" secondary @click="addTask">添加</NButton>
      </div>
      <p v-if="err" class="err">{{ err }}</p>
      <div class="board">
        <TaskBoard :tasks="tasks" @reload="load" />
      </div>
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
    <section class="glass-card">
      <div class="modhead">
        <h2>提醒规则（到点弹卡 / 全屏）</h2>
        <div class="modacts">
          <NButton size="tiny" quaternary @click="importRules">导入 JSON</NButton>
          <NButton size="tiny" quaternary @click="exportRules">导出 JSON</NButton>
        </div>
      </div>
      <ReminderRules :rules="rules" @reload="load" />
    </section>
    <!-- 某天详情（二级视图，先搭框架）。Teleport 到 body：卡片的 backdrop-filter 会创建包含块，
         不 Teleport 的话 position:fixed 会被困在卡片内部（表现成"同级卡片"） -->
    <Teleport to="body">
      <div v-show="detailDate" class="mask" @click.self="detailDate = ''">
      <div class="sheet glass-card" :class="{ big: sheetBig }">
        <div class="modhead">
          <h2>{{ detailDate }} · 那天</h2>
          <div class="dacts">
            <button class="dbtn" :title="sheetBig ? '还原' : '放大'" @click="sheetBig = !sheetBig">
              <Icon :name="sheetBig ? 'collapse' : 'expand'" :size="15" />
            </button>
            <button class="dbtn" title="关闭" @click="detailDate = ''">
              <Icon name="close" :size="15" />
            </button>
          </div>
        </div>
        <DayDetail :date="detailDate" />
      </div>
      </div>
    </Teleport>

    <!-- 每日日志 -->
    <section class="glass-card">
      <h2>日志（Markdown + 图片）</h2>
      <NotesPanel :day-ts="pickedDay" />
    </section>
  </div>
</template>

<style scoped>
.todo {
  display: flex;
  flex-direction: column;
  gap: 14px;
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
  inset: 16px;
  z-index: 40;
  /* 遮罩用主题底色而不是纯黑：浅色模式下不再"压黑" */
  /* 背景更重的毛玻璃：遮罩本身也做模糊，突出二级界面 */
  background: rgba(var(--bg-rgb), 0.42);
  backdrop-filter: blur(26px) saturate(1.2);
  -webkit-backdrop-filter: blur(26px) saturate(1.2);
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.dacts {
  display: flex;
  align-items: center;
  gap: 2px;
}

.dbtn {
  width: 26px;
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
  width: min(720px, 100%);
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
