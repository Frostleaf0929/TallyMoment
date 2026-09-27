<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NSlider, NSwitch } from "naive-ui";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { appsTopN } from "../lib/uiState";

interface DataInfo {
  dir: string;
  dbPath: string;
  dbBytes: number;
  walBytes: number;
  fallback: boolean;
  exists: boolean;
}

const autoStart = ref(false);
const data = ref<DataInfo | null>(null);
const msg = ref("");

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

async function saveTopN(v: number) {
  try {
    const p = await invoke<{ appsTopN: number }>("prefs_set", { appsTopN: v });
    appsTopN.value = p.appsTopN;
  } catch {
    /* 忽略 */
  }
}

onMounted(async () => {
  try {
    autoStart.value = await isEnabled();
  } catch {
    autoStart.value = false;
  }
  try {
    data.value = await invoke<DataInfo>("data_info");
  } catch {
    /* 忽略 */
  }
});
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
      <div class="row">
        <div class="rlabel">
          <p class="rt">记录</p>
          <p class="rd">记录开关在托盘右键菜单里（暂停记录 / 显示桌宠）</p>
        </div>
      </div>
    </div>

    <div class="glass-card card">
      <h2>显示</h2>
      <div class="row col">
        <div class="rlabel">
          <p class="rt">应用排行显示条数</p>
          <p class="rd">5 ~ 20 条，默认 10 条</p>
        </div>
        <NSlider
          :value="appsTopN"
          :min="5"
          :max="20"
          :step="1"
          :format-tooltip="(v: number) => v + ' 条'"
          style="max-width: 320px"
          @update:value="(v: number) => (appsTopN = v)"
          @change="saveTopN"
        />
      </div>
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
        <p class="kv"><span>版本</span><b>0.1.0</b></p>
        <p class="kv"><span>数据</span><b>本地优先，不联网、不上传</b></p>
      </div>
      <p class="rd more">
        图标：Solar Line Duotone（480 Design，CC BY 4.0，经由 Iconify）。<br />
        桌宠：Bongo Cat Mver 兼容模型；内置素材为原版 BongoCat 分层图，
        「兔子洞」皮肤为 Live2D 素材（版权归原作者，仅个人使用）。
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

.err {
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--danger);
}
</style>
