<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NInput } from "naive-ui";

interface Summary {
  apps: number;
  dailyRows: number;
  hourlyRows: number;
  segments: number;
  skipped: number;
}

const taiPath = ref("");
const restorePath = ref("");
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
    if (!taiPath.value.trim()) throw new Error("请先填写 Tai 数据库路径");
    const s = await invoke<Summary>("import_tai", { path: taiPath.value.trim() });
    return `导入完成：应用 ${s.apps} 个，日汇总 ${s.dailyRows} 行，小时汇总 ${s.hourlyRows} 行，跳过 ${s.skipped} 条无效数据`;
  });

const doExport = () =>
  run("export", async () => {
    const p = await invoke<string>("export_json");
    return `已导出：${p}`;
  });

const doRestore = () =>
  run("restore", async () => {
    if (!restorePath.value.trim()) throw new Error("请先填写导出文件路径");
    const s = await invoke<Summary>("restore_json", { path: restorePath.value.trim() });
    return `恢复完成：应用 ${s.apps} 个，区间 ${s.segments} 条，日汇总 ${s.dailyRows} 行，小时汇总 ${s.hourlyRows} 行`;
  });
</script>

<template>
  <div class="data">
    <div class="block">
      <p class="t">从 Tai 导入</p>
      <p class="d">填 Tai 安装目录下 Data\data.db 的完整路径，导入后应用与历史时长会合并进拾刻（同一文件只导入一次）</p>
      <div class="line">
        <NInput v-model:value="taiPath" size="small" placeholder="例如 D:\Tools\Tai\Data\data.db" />
        <NButton size="small" type="primary" secondary :loading="busy === 'tai'" @click="importTai">
          导入
        </NButton>
      </div>
    </div>

    <div class="block">
      <p class="t">导出全部数据（JSON）</p>
      <p class="d">把应用、区间、汇总、清单一次性导出为 JSON 文件，存放在数据目录</p>
      <NButton size="small" secondary :loading="busy === 'export'" @click="doExport">导出</NButton>
    </div>

    <div class="block">
      <p class="t">从导出文件恢复</p>
      <p class="d">⚠ 仅当当前数据库为空时可用（换机/重装后的全新恢复），避免数据叠加</p>
      <div class="line">
        <NInput v-model:value="restorePath" size="small" placeholder="导出文件完整路径，如 D:\...\export-20260926-120000.json" />
        <NButton size="small" type="warning" secondary :loading="busy === 'restore'" @click="doRestore">
          恢复
        </NButton>
      </div>
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

.line {
  display: flex;
  gap: 8px;
}

.line > :first-child {
  flex: 1;
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
