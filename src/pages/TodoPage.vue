<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NInput, NSelect } from "naive-ui";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { ReminderRule, Task, TodoStats } from "../types";
import Icon from "../components/Icon.vue";
import TaskBoard from "../components/TaskBoard.vue";
import ReminderRules from "../components/ReminderRules.vue";

const tasks = ref<Task[]>([]);
const rules = ref<ReminderRule[]>([]);
const stats = ref<TodoStats | null>(null);

const newContent = ref("");
const newPriority = ref(1);
const newDue = ref<string | null>(null);
const err = ref("");
const msg = ref("");

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
        <NButton size="small" secondary @click="importMd">导入 MD</NButton>
        <NButton size="small" secondary @click="exportMd">导出 MD</NButton>
      </div>
    </header>
    <p v-if="msg" class="okline">{{ msg }}</p>

    <!-- 完成率统计 -->
    <section class="cards">
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
        <p class="value small">{{ stats?.avgMinutes ? `${stats.avgMinutes} 分钟` : "—" }}</p>
      </div>
    </section>

    <!-- 任务看板 -->
    <section class="glass-card">
      <h2>任务</h2>
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

    <!-- 完成区间分布 -->
    <section class="glass-card">
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
      <h2>提醒规则（到点在右下角弹卡）</h2>
      <ReminderRules :rules="rules" @reload="load" />
    </section>
  </div>
</template>

<style scoped>
.todo {
  display: flex;
  flex-direction: column;
  gap: 14px;
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
