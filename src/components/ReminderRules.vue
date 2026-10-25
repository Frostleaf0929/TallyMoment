<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openFile, save as saveFile } from "@tauri-apps/plugin-dialog";
import { NButton, NPopconfirm, NSwitch } from "naive-ui";
import type { ReminderRule } from "../types";

const props = defineProps<{ rules: ReminderRule[] }>();
const emit = defineEmits<{ (e: "reload"): void }>();

/* ---------- 列表 / 次级表单（与任务卡同一套交互：列表只负责看，编辑进表单） ---------- */
const form = ref<null | { isNew: boolean; id: number }>(null);
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
  /** 生效范围：空 = 不限（比如"就这几天提醒"设个结束日期即可） */
  startDate: "",
  endDate: "",
});
const err = ref("");

function openNew() {
  err.value = "";
  form.value = { isNew: true, id: -1 };
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
    startDate: "",
    endDate: "",
  };
}

function openEdit(r: ReminderRule) {
  err.value = "";
  form.value = { isNew: false, id: r.id };
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
    startDate: r.startDate ?? "",
    endDate: r.endDate ?? "",
  };
}

function closeForm() {
  form.value = null;
  err.value = "";
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
    startDate: draft.value.startDate || null,
    endDate: draft.value.endDate || null,
  };
  try {
    const f = form.value;
    if (!f) return;
    await invoke(f.isNew ? "rule_add" : "rule_update", f.isNew ? args : { id: f.id, ...args });
    closeForm();
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

/** 生效范围标签：10/24~10/31（只有一端就显示 起/止） */
const rangeLabel = (r: ReminderRule): string => {
  if (!r.startDate && !r.endDate) return "";
  const md = (s: string | null) => (s ? s.slice(5).replace("-", "/") : "");
  if (r.startDate && r.endDate) return `${md(r.startDate)}~${md(r.endDate)}`;
  if (r.startDate) return `${md(r.startDate)} 起`;
  return `至 ${md(r.endDate)}`;
};

/* ---------- JSON 导入导出（低频，收在列表头部） ---------- */
async function exportRules() {
  err.value = "";
  try {
    const picked = await saveFile({
      title: "导出提醒规则",
      defaultPath: `拾刻提醒规则-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!picked) return;
    await invoke<string>("rules_export_json", { path: picked });
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function importRules() {
  err.value = "";
  try {
    const picked = await openFile({
      multiple: false,
      directory: false,
      title: "选择规则 JSON",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    await invoke<[number, number]>("rules_import_json", { path: picked });
    emit("reload");
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
</script>

<template>
  <div>
    <!-- ===== 列表视图 ===== -->
    <template v-if="!form">
      <div class="listhead">
        <span class="lstat">{{ rules.filter((r) => r.enabled).length }} 条启用中 · 共 {{ rules.length }} 条</span>
        <div class="lacts">
          <NButton size="tiny" quaternary title="从 JSON 导入规则" @click="importRules">导入 JSON</NButton>
          <NButton size="tiny" quaternary title="导出规则为 JSON" @click="exportRules">导出 JSON</NButton>
          <button class="newbtn" @click="openNew">
            <svg viewBox="0 0 12 12" aria-hidden="true">
              <path d="M6 2v8M2 6h8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
            </svg>
            新建提醒
          </button>
        </div>
      </div>

      <div v-for="r in rules" :key="r.id" class="row" :class="{ off: !r.enabled }">
        <span class="dot" :class="{ on: r.enabled }" :title="r.enabled ? '启用中' : '已停用'"></span>
        <span class="content" :title="r.title">{{ r.title }}</span>
        <span v-if="r.body" class="body" :title="r.body">{{ r.body }}</span>
        <span class="spacer"></span>
        <span class="tag">{{ modeLabel(r) }}</span>
        <span class="tag">{{ r.sticky ? "常驻" : `${r.cardDurationSec}s` }}</span>
        <span class="tag" :class="{ full: r.style === 'fullscreen' }">
          {{ r.style === "fullscreen" ? "全屏" : "卡片" }}
        </span>
        <span v-if="rangeLabel(r)" class="tag rng" title="生效日期范围，过期自动停">{{ rangeLabel(r) }}</span>
        <NSwitch size="small" :value="r.enabled" @update:value="(v: boolean) => toggle(r, v)" />
        <button class="mini edit" title="编辑" @click="openEdit(r)">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 20h4L19 9l-4-4L4 16v4zM13 6l4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>编辑</span>
        </button>
        <NPopconfirm @positive-click="remove(r)">
          <template #trigger>
            <NButton quaternary size="tiny" type="error">删除</NButton>
          </template>
          删除提醒「{{ r.title }}」？
        </NPopconfirm>
      </div>
      <p v-if="!rules.length" class="empty">还没有提醒规则，点右上角「新建提醒」开始</p>
    </template>

    <!-- ===== 次级表单 ===== -->
    <template v-else>
      <div class="formhead">
        <button class="mini back" @click="closeForm">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M14.5 5.5L8 12l6.5 6.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          返回列表
        </button>
        <span class="ftitle">{{ form.isNew ? "新建提醒" : "编辑提醒" }}</span>
      </div>

      <input v-model="draft.title" class="finput" placeholder="标题，如：护眼提醒" maxlength="50" />
      <textarea v-model="draft.body" class="finput area" placeholder="正文（可选）" rows="2" />
      <div class="frow">
        <span class="flabel">触发</span>
        <div class="seg">
          <button class="seg-i" :class="{ on: draft.mode === 'interval' }" @click="draft.mode = 'interval'">时间间隔</button>
          <button class="seg-i" :class="{ on: draft.mode === 'daily' }" @click="draft.mode = 'daily'">每日定点</button>
        </div>
        <span class="flabel">方式</span>
        <div class="seg">
          <button class="seg-i" :class="{ on: draft.style === 'card' }" @click="draft.style = 'card'">右下角卡片</button>
          <button class="seg-i" :class="{ on: draft.style === 'fullscreen' }" @click="draft.style = 'fullscreen'">全屏提醒</button>
        </div>
      </div>
      <div v-if="draft.mode === 'interval'" class="frow">
        <span>每</span>
        <input v-model.number="draft.intervalMinutes" type="number" min="1" max="1440" class="num" />
        <span>分钟提醒一次（1~1440）</span>
      </div>
      <div v-else class="frow">
        <input v-model="draft.dailyTime" type="time" class="num tp" />
        <button class="mini" @click="addTime">添加时间点</button>
        <span v-if="draft.dailyTimes.length" class="times">{{ draft.dailyTimes.join(" / ") }}</span>
      </div>
      <div class="frow">
        <span>停留</span>
        <input v-model.number="draft.cardDurationSec" type="number" min="3" max="600" class="num" />
        <span>秒</span>
        <label class="stk"><input v-model="draft.sticky" type="checkbox" /> 卡片常驻</label>
      </div>
      <div class="frow">
        <span>生效范围</span>
        <input v-model="draft.startDate" type="date" class="num tp" title="从这天开始提醒（留空 = 不限）" />
        <span>~</span>
        <input v-model="draft.endDate" type="date" class="num tp" title="到这天为止（留空 = 一直提醒）" />
        <span class="rangehint">两端都留空 = 一直生效；设个结束日期就不用手动停了</span>
      </div>
      <p v-if="err" class="ferr">{{ err }}</p>
      <div class="btns">
        <button class="mini" @click="closeForm">取消</button>
        <button class="primary" @click="save">{{ form.isNew ? "创建" : "保存" }}</button>
      </div>
    </template>
  </div>
</template>

<style scoped>
/* 与任务卡同一套规格：行/标签/小按钮共用尺寸 */
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

.row.off {
  opacity: 0.5;
  border-style: dashed;
}

.dot {
  flex: none;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--border);
}

.dot.on {
  background: var(--good);
}

.content {
  /* 标题与备注都限宽：超出省略，长规则不挤压右侧标签 */
  flex: none;
  max-width: 210px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.body {
  flex: none;
  max-width: 280px;
  font-size: 11.5px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.spacer {
  flex: 1;
}

.tag {
  flex: none;
  font-size: 10px;
  color: var(--accent-text);
  background: var(--accent-soft);
  border-radius: var(--r-full);
  padding: 1px 8px;
}

.tag.full {
  color: var(--danger);
  background: var(--danger-soft);
}

.tag.rng {
  color: var(--info, #6aa1ff);
  background: var(--info-soft, rgba(106, 161, 255, 0.14));
}

.mini {
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-faint);
  border-radius: var(--r-full);
  font-size: 10px;
  font-family: inherit;
  padding: 2px 9px;
  cursor: pointer;
  transition: color var(--dur), border-color var(--dur), background var(--dur);
}

.mini svg {
  width: 11px;
  height: 11px;
  flex: none;
}

/* 编辑胶囊：与日志卡同款——默认圆形图标，悬停展开文字 */
.mini.edit {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0;
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

.empty {
  font-size: 12px;
  color: var(--text-faint);
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
  box-sizing: border-box;
}

.finput:focus {
  border-color: var(--accent-text);
}

.finput.area {
  resize: vertical;
  line-height: 1.6;
}

.frow {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 10px;
  font-size: 12px;
  color: var(--text-muted);
}

.flabel {
  font-size: 11px;
  color: var(--text-muted, #888);
}

.seg {
  display: inline-flex;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  padding: 3px;
  background: var(--surface);
}

.seg-i {
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 12px;
  font-family: inherit;
  padding: 5px 12px;
  border-radius: var(--r-sm);
  cursor: pointer;
}

.seg-i.on {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.num {
  width: 70px;
  border: 1px solid var(--border-strong);
  background: var(--surface);
  color: var(--text);
  border-radius: 8px;
  padding: 5px 7px;
  font-size: 12px;
  font-family: inherit;
  outline: none;
}

.num.tp {
  width: auto;
}

.times {
  color: var(--accent-text);
}

.rangehint {
  font-size: 11px;
  color: var(--text-faint);
}

.stk {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
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

.ferr {
  margin: 0;
  font-size: 12px;
  color: var(--danger);
}

.btns {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
