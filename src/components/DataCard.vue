<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { NButton } from "naive-ui";

interface Summary {
  apps: number;
  dailyRows: number;
  hourlyRows: number;
  segments: number;
  skipped: number;
}

const busy = ref("");
const okMsg = ref("");
const errMsg = ref("");

async function run(label: string, fn: () => Promise<string>) {
  busy.value = label;
  okMsg.value = "";
  errMsg.value = "";
  try {
    okMsg.value = await fn();
  } catch (e) {
    errMsg.value = String(e).replace(/^.*Error: /, "");
  } finally {
    busy.value = "";
  }
}

const importTai = () =>
  run("tai", async () => {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择 Tai 的 data.db",
      filters: [{ name: "Tai 数据库", extensions: ["db"] }],
    });
    if (!picked) return "已取消";
    const s = await invoke<Summary>("import_tai", { path: picked });
    return `导入完成：应用 ${s.apps} 个，日汇总 ${s.dailyRows} 行，小时汇总 ${s.hourlyRows} 行，跳过 ${s.skipped} 条无效数据`;
  });

const doExport = () =>
  run("export", async () => {
    const target = await save({
      title: "选择导出位置",
      defaultPath: `export-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!target) return "已取消";
    const p = await invoke<string>("export_json", { path: target });
    return `已导出：${p}`;
  });

const doRestore = () =>
  run("restore", async () => {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择拾刻导出的 JSON 文件",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!picked) return "已取消";
    const s = await invoke<Summary>("restore_json", { path: picked });
    return `恢复完成：应用 ${s.apps} 个，区间 ${s.segments} 条，日汇总 ${s.dailyRows} 行，小时汇总 ${s.hourlyRows} 行`;
  });
</script>

<template>
  <div class="data">
    <div class="block">
      <p class="t">从 Tai 导入</p>
      <p class="d">选择 Tai 安装目录下 Data\data.db，应用与历史时长会合并进拾刻（同一文件只导入一次）</p>
      <NButton size="small" type="primary" secondary :loading="busy === 'tai'" @click="importTai">
        选择文件并导入
      </NButton>
    </div>

    <div class="block">
      <p class="t">导出全部数据（JSON）</p>
      <p class="d">把应用、区间、汇总、清单一次性导出为 JSON 文件，位置自选</p>
      <NButton size="small" secondary :loading="busy === 'export'" @click="doExport">选择位置并导出</NButton>
    </div>

    <div class="block">
      <p class="t">从导出文件恢复</p>
      <p class="d">⚠ 仅当当前数据库为空时可用（换机/重装后的全新恢复），避免数据叠加</p>
      <NButton size="small" type="warning" secondary :loading="busy === 'restore'" @click="doRestore">
        选择文件并恢复
      </NButton>
    </div>

    <p v-if="okMsg" class="ok">{{ okMsg }}</p>
    <p v-if="errMsg" class="err">{{ errMsg }}</p>
  </div>
</template>

<style scoped>
.data {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.block .t {
  margin: 0 0 2px;
  font-size: 13px;
  font-weight: 600;
  color: #d3d8e4;
}

.block .d {
  margin: 0 0 8px;
  font-size: 12px;
  color: #6b7280;
}

.ok {
  margin: 0;
  font-size: 12px;
  color: #34d399;
  word-break: break-all;
}

.err {
  margin: 0;
  font-size: 12px;
  color: #f87171;
}
</style>
