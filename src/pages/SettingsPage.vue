<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NSwitch } from "naive-ui";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import DataCard from "../components/DataCard.vue";

interface DataInfo {
  dir: string;
  dbPath: string;
  dbBytes: number;
  walBytes: number;
  fallback: boolean;
  exists: boolean;
}

/** 开发模式下自启动会因缺少 dev 服务器而报连接错误，界面上直接说明 */
const isDev = import.meta.env.DEV;
const autoStart = ref(false);
const trackSelf = ref(false);
type DataMod = "tasks" | "rules" | "all";
const mod = ref<DataMod>("tasks");
const dataMsg = ref("");
const dataErr = ref("");
const stamp = () => new Date().toISOString().slice(0, 10);

/** Tai 时间数据导入模式：fill=只补没有的 / merge=时长累加 / replace=清空后导 */
const taiMode = ref<"fill" | "merge" | "replace">("fill");
const taiMsg = ref("");
const taiErr = ref("");

async function importTaiNow() {
  taiMsg.value = "";
  taiErr.value = "";
  try {
    const p = await open({
      multiple: false,
      directory: false,
      title: "选择 Tai 的 data.db",
      filters: [{ name: "SQLite 数据库", extensions: ["db", "sqlite"] }],
    });
    if (!p || Array.isArray(p)) return;
    if (taiMode.value === "replace") {
      const sure = await ask(
        "将先清空本地全部时间记录（使用明细、日汇总、时段汇总），再以所选文件为准导入；键鼠计数、任务、提醒规则、日志都会保留。确定继续？",
        { title: "替代全部时间数据", kind: "warning" }
      );
      if (!sure) return;
    }
    const s = await invoke<{
      apps: number;
      dailyRows: number;
      hourlyRows: number;
      skipped: number;
      cleared: number;
    }>("import_tai", { path: p, mode: taiMode.value });
    const parts = [`应用 ${s.apps} 个`, `日汇总 ${s.dailyRows} 行`, `时段 ${s.hourlyRows} 行`];
    if (s.cleared > 0) parts.push(`已清空本地时间数据 ${s.cleared} 行`);
    if (s.skipped > 0) parts.push(`跳过已有 ${s.skipped} 条`);
    taiMsg.value = `导入完成：${parts.join("，")}`;
  } catch (e) {
    taiErr.value = String(e).replace(/^.*Error: /, "");
  }
}

/** 导出所选模块 */
async function exportModule() {
  dataMsg.value = "";
  dataErr.value = "";
  try {
    if (mod.value === "tasks") {
      const p = await save({
        title: "导出任务（Markdown）",
        defaultPath: `拾刻待办-${stamp()}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!p) return;
      const f = await invoke<string>("tasks_export_md", { path: p });
      dataMsg.value = `已导出任务：${f}`;
      return;
    }
    if (mod.value === "rules") {
      const p = await save({
        title: "导出提醒规则（JSON）",
        defaultPath: `拾刻提醒规则-${stamp()}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!p) return;
      const f = await invoke<string>("rules_export_json", { path: p });
      dataMsg.value = `已导出规则：${f}`;
      return;
    }
    const p = await save({
      title: "整包导出（任务 + 提醒规则）",
      defaultPath: `拾刻导出-${stamp()}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!p) return;
    const dir = String(p).replace(/[^\/]*$/, "");
    await invoke("tasks_export_md", { path: p });
    await invoke("rules_export_json", { path: `${dir}拾刻提醒规则-${stamp()}.json` });
    dataMsg.value = `已整包导出到：${dir}`;
  } catch (e) {
    dataErr.value = String(e).replace(/^.*Error: /, "");
  }
}

/** 导入：按扩展名自动识别是任务(md)还是规则(json) */
async function importModule() {
  dataMsg.value = "";
  dataErr.value = "";
  try {
    const p = await open({
      multiple: false,
      directory: false,
      title: "选择要导入的文件",
      filters: [{ name: "Markdown / JSON", extensions: ["md", "markdown", "json", "txt"] }],
    });
    if (!p || Array.isArray(p)) return;
    const lower = String(p).toLowerCase();
    if (lower.endsWith(".json")) {
      const [added, skipped] = await invoke<[number, number]>("rules_import_json", { path: p });
      dataMsg.value = `导入提醒规则 ${added} 条，跳过同名 ${skipped} 条`;
    } else {
      const sum = await invoke<{ skipped: number }>("tasks_import_md", { path: p });
      dataMsg.value = `导入任务完成，跳过 ${sum.skipped} 条已存在的`;
    }
  } catch (e) {
    dataErr.value = String(e).replace(/^.*Error: /, "");
  }
}
const data = ref<DataInfo | null>(null);
const msg = ref("");

async function toggleTrackSelf(v: boolean) {
  try {
    await invoke("set_track_self", { on: v });
    trackSelf.value = v;
  } catch (e) {
    dataErr.value = String(e).replace(/^.*Error: /, "");
  }
}

async function toggleAutoStart(v: boolean) {
  try {
    if (v) await enable();
    else await disable();
    autoStart.value = v;
  } catch {
    autoStart.value = false;
  }
}

async function openDataDir() {
  msg.value = "";
  try {
    await invoke("open_data_dir");
  } catch (e) {
    msg.value = String(e).replace(/^.*Error: /, "");
  }
}

function fmtSize(bytes: number): string {
  if (bytes <= 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}


onMounted(async () => {
  try {
    autoStart.value = await isEnabled();
  } catch {
    autoStart.value = false;
  }
  try {
    trackSelf.value = await invoke<boolean>("get_track_self");
  } catch {
    trackSelf.value = false;
  }
  try {
    data.value = await invoke<DataInfo>("data_info");
  } catch {
    /* 忽略 */
  }
  try {
    flow.value = await invoke<{ blipSecs: number; switchBase: number; switchPer10min: number }>(
      "flow_get_params"
    );
  } catch {
    /* 忽略 */
  }
});

/** 心流判定参数（设置页步进，即刻落库，洞察页下次加载即生效） */
const flow = ref({ blipSecs: 45, switchBase: 3, switchPer10min: 1 });
const flowErr = ref("");
async function bumpFlow(key: "blipSecs" | "switchBase" | "switchPer10min", delta: number) {
  const limits: Record<string, [number, number, number]> = {
    blipSecs: [5, 0, 300],
    switchBase: [1, 1, 10],
    switchPer10min: [1, 0, 5],
  };
  const [step, lo, hi] = limits[key];
  const v = Math.min(hi, Math.max(lo, flow.value[key] + delta * step));
  try {
    await invoke("flow_set_params", {
      blipSecs: key === "blipSecs" ? v : flow.value.blipSecs,
      switchBase: key === "switchBase" ? v : flow.value.switchBase,
      switchPer10min: key === "switchPer10min" ? v : flow.value.switchPer10min,
    });
    flow.value = { ...flow.value, [key]: v };
    flowErr.value = "";
  } catch (e) {
    flowErr.value = String(e).replace(/^.*Error: /, "");
  }
}
</script>

<template>
  <div class="page">
    <header class="phead">
      <h1>设置</h1>
      <span class="sub">系统行为、数据与关于</span>
    </header>

    <div class="glass-card card">
      <h2>系统</h2>
      <div class="row">
        <div class="rlabel">
          <p class="rt">开机自启动</p>
          <p class="rd">登录 Windows 后自动在后台运行并开始记录</p>
        </div>
        <NSwitch :value="autoStart" @update:value="toggleAutoStart" />
      </div>
      <p v-if="isDev" class="rd more">
        注意：当前是<b>开发模式</b>（pnpm tauri dev）。自启动会拉起开发版程序，而开发版的界面是从本地开发服务器加载的，
        开机时那个服务器没在运行，所以会弹出「无法访问此页面 / localhost 拒绝连接」——这是开发模式的必然现象，
        不是自启动没注册。打包之后（pnpm tauri build）界面从程序自带文件加载，自启动就正常了。
      </p>
      <div class="row">
        <div class="rlabel">
          <p class="rt">记录拾刻自身</p>
          <p class="rd">默认关闭：弹提醒卡会把拾刻置为前台，计进去会污染应用统计；打开后拾刻自己的前台时间也正常记录</p>
        </div>
        <NSwitch :value="trackSelf" @update:value="toggleTrackSelf" />
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">记录</p>
          <p class="rd">记录开关在托盘右键菜单里（暂停记录 / 显示桌宠）</p>
        </div>
      </div>
    </div>

    <div class="glass-card card">
      <h2>心流判定</h2>
      <div class="row">
        <div class="rlabel">
          <p class="rt">过场宽限</p>
          <p class="rd">块内不超过该秒数的短段不算打断（"切走一小会儿就切回来"），0 = 关闭宽限</p>
        </div>
        <div class="stepper">
          <button class="mini" @click="bumpFlow('blipSecs', -5)">−</button>
          <span class="sval">{{ flow.blipSecs }} 秒</span>
          <button class="mini" @click="bumpFlow('blipSecs', 5)">+</button>
        </div>
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">切换上限</p>
          <p class="rd">基线次数 + 每 10 分钟放宽次数；超过则该段判为"专注"而非心流</p>
        </div>
        <div class="stepper">
          <button class="mini" @click="bumpFlow('switchBase', -1)">−</button>
          <span class="sval">{{ flow.switchBase }} 次</span>
          <button class="mini" @click="bumpFlow('switchBase', 1)">+</button>
        </div>
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">每 10 分钟放宽</p>
          <p class="rd">块每长 10 分钟，切换上限额外 +N（多应用协同调大些）</p>
        </div>
        <div class="stepper">
          <button class="mini" @click="bumpFlow('switchPer10min', -1)">−</button>
          <span class="sval">{{ flow.switchPer10min }} 次</span>
          <button class="mini" @click="bumpFlow('switchPer10min', 1)">+</button>
        </div>
      </div>
      <p class="rd more">改动即刻生效，洞察页的心流/专注划分会按新参数重新计算。</p>
      <p v-if="flowErr" class="err">{{ flowErr }}</p>
    </div>

    <div class="glass-card card">
      <h2>数据</h2>
      <div class="rows">
        <p class="kv"><span>存放位置</span><b>{{ data?.dir ?? "读取中…" }}</b></p>
        <p class="kv"><span>数据库</span><b>{{ fmtSize(data?.dbBytes ?? 0) }}</b></p>
        <p class="kv"><span>未合并日志(WAL)</span><b>{{ fmtSize(data?.walBytes ?? 0) }}</b></p>
        <p class="kv">
          <span>位置来源</span>
          <b>{{ data?.fallback ? "程序目录不可写，已回退到用户目录" : "程序目录旁的 Data 文件夹" }}</b>
        </p>
      </div>
      <div class="acts">
        <button class="btn" @click="openDataDir">打开数据目录</button>
      </div>
      <!-- 分模块导入导出（日志随日志功能上线后加入） -->
      <!-- Tai 时间数据导入：三种合并策略 -->
      <div class="datacard">
        <p class="rt">导入时间数据（Tai 的 data.db）</p>
        <div class="seg">
          <button class="seg-item" :class="{ active: taiMode === 'fill' }" @click="taiMode = 'fill'">补充去重</button>
          <button class="seg-item" :class="{ active: taiMode === 'merge' }" @click="taiMode = 'merge'">合并累加</button>
          <button class="seg-item" :class="{ active: taiMode === 'replace' }" @click="taiMode = 'replace'">替代全部</button>
        </div>
        <p class="rd more">
          <b>补充去重</b>：只导入本地没有的日期/时段，已有的不覆盖、不叠加（推荐）。<br />
          <b>合并累加</b>：时长与本地相加——同一文件不会重复导入，但换一个文件再导同一段时间会把时长加两遍。<br />
          <b>替代全部</b>：先清空本地全部时间记录，以导入文件为准。键鼠计数、任务、提醒规则、日志在任何模式下都保留。
        </p>
        <div class="acts">
          <button class="btn" @click="importTaiNow">选择 data.db 并导入</button>
        </div>
        <p v-if="taiMsg" class="okline">{{ taiMsg }}</p>
        <p v-if="taiErr" class="err">{{ taiErr }}</p>
      </div>

      <div class="datacard">
        <p class="rt">分模块导入 / 导出</p>
        <div class="seg">
          <button
            class="seg-item"
            :class="{ active: mod === 'tasks' }"
            @click="mod = 'tasks'"
          >
            任务
          </button>
          <button
            class="seg-item"
            :class="{ active: mod === 'rules' }"
            @click="mod = 'rules'"
          >
            提醒规则
          </button>
          <button class="seg-item" :class="{ active: mod === 'all' }" @click="mod = 'all'">
            全部待办
          </button>
        </div>
        <div class="acts">
          <button class="btn" @click="exportModule">导出所选模块</button>
          <button class="btn" @click="importModule">导入（按文件类型自动识别）</button>
        </div>
        <p class="rd more">
          任务用 <b>Markdown</b>（Obsidian / Notion 可直接打开，行尾带回优先级与到期）；
          提醒规则用 <b>JSON</b>（间隔、定点时间、卡片时长等结构化字段 Markdown 表达不了）；
          「每日日志」的导入导出随日志功能一起上线。
        </p>
        <p v-if="dataMsg" class="okline">{{ dataMsg }}</p>
        <p v-if="dataErr" class="err">{{ dataErr }}</p>
      </div>

      <div class="datacard">
        <DataCard />
      </div>
      <p class="rd more">
        数据只存在本机、不上传。<b>崩溃安全</b>：切换应用、离开键盘 60 秒、退出程序时都会立即落库；
        长会话每分钟打一次检查点；键鼠计数每 5 秒落一次。也就是说意外崩溃最多损失 1 分钟记录，
        已落库的数据在 WAL 日志保护下不会损坏。
      </p>
      <p v-if="msg" class="err">{{ msg }}</p>
    </div>

    <div class="glass-card card">
      <h2>关于</h2>
      <div class="rows">
        <p class="kv"><span>名称</span><b>拾刻 · TallyMoment</b></p>
        <p class="kv"><span>版本</span><b>0.2.0</b></p>
        <p class="kv"><span>数据</span><b>本地优先，不联网、不上传</b></p>
      </div>
      <p class="rd more">
        图标：Solar Line Duotone（480 Design, CC BY 4.0，来自 svgrepo 图标库）。<br />
        动效启发：uiverse.io ｜ 借鉴：Tai、Catrace ｜ 灵感：flow-insight<br />
        本软件为 vibe coding 产物，由 GLM-5.3-flash 与 DeepSeek-V4.1 在 ZCode 中开发。
      </p>
      <p class="rd more">
        软件名与图标会写进 exe 与安装包（任务栏、后台视图显示的就是这个）；
        改名需要重新打包，属于打包批次的工作。
      </p>
    </div>
  </div>
</template>

<style scoped>
.page {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.phead {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.phead h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
}

.sub {
  font-size: 12px;
  color: var(--text-muted);
}

.card {
  padding: 16px 18px;
}

.card h2 {
  margin: 0 0 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 6px 0;
}

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

.row.col {
  flex-direction: column;
  align-items: stretch;
  gap: 8px;
}

.rlabel .rt {
  margin: 0 0 2px;
  font-size: 14px;
  color: var(--text);
}

.rlabel .rd {
  margin: 0;
  font-size: 12px;
  color: var(--text-muted);
}

.rows {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.kv {
  margin: 0;
  display: flex;
  gap: 12px;
  font-size: 12px;
  color: var(--text-muted);
}

.kv span {
  flex: none;
  width: 116px;
  color: var(--text-faint);
}

.kv b {
  color: var(--text);
  font-weight: 500;
  word-break: break-all;
}

.acts {
  margin-top: 12px;
}

/* 原来「数据」页的导入/导出/恢复/删除，直接并进这张卡片 */
.datacard {
  margin-top: 14px;
  padding-top: 12px;
  border-top: 1px solid var(--border);
}

.btn {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  border-radius: var(--r-sm);
  font-size: 12px;
  font-family: inherit;
  padding: 6px 14px;
  cursor: pointer;
}

.btn:hover {
  background: var(--surface-hover);
}

.rd.more {
  margin: 10px 0 0;
  font-size: 11.5px;
  line-height: 1.7;
  color: var(--text-faint);
}

.okline {
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--good);
  word-break: break-all;
}

.err {
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--danger);
}

/* 心流判定参数步进器 */
.stepper {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: none;
}
.mini {
  width: 26px;
  height: 24px;
  font-size: 13px;
  line-height: 1;
  border: 1px solid rgba(128, 128, 128, 0.35);
  border-radius: 6px;
  background: none;
  color: var(--text, #ddd);
  cursor: pointer;
}
.mini:hover {
  border-color: var(--text, #ddd);
}
.sval {
  min-width: 56px;
  text-align: center;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  color: var(--text, #ddd);
}
</style>
