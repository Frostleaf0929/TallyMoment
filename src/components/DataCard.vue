<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { NButton, NModal, NSelect } from "naive-ui";

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

// 删除数据
const showDelete = ref(false);
const deleteScope = ref("today");
const deleteBusy = ref(false);
const deleteScopeOptions = [
  { label: "仅今天的时间记录", value: "today" },
  { label: "全部时间记录（任务与提醒不受影响）", value: "all" },
];

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

const exportJson = () =>
  run("export", async () => {
    const target = await save({
      title: "选择导出位置",
      defaultPath: `拾刻备份-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!target) return "已取消";
    const p = await invoke<string>("export_json", { path: target });
    return `已导出：${p}`;
  });

const restoreJson = () =>
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

const exportTai = () =>
  run("tai", async () => {
    const target = await save({
      title: "选择 Tai 数据库导出位置",
      defaultPath: "Tai数据.db",
      filters: [{ name: "SQLite 数据库", extensions: ["db"] }],
    });
    if (!target) return "已取消";
    const files = await invoke<string[]>("export_tai", { path: target });
    return `已生成 ${files.length} 个文件：\n${files.join("\n")}\n把 .db 放进 Tai 的 Data 目录即可在 Tai 中使用`;
  });

async function doDelete() {
  deleteBusy.value = true;
  errMsg.value = "";
  try {
    const s = await invoke<{
      segments: number;
      hourly: number;
      daily: number;
      input: number;
      apps: number;
    }>("delete_data", { scope: deleteScope.value, appId: null });
    okMsg.value = `已删除：区间 ${s.segments} 条、小时汇总 ${s.hourly} 行、日汇总 ${s.daily} 行、键鼠 ${s.input} 行、应用 ${s.apps} 个`;
    showDelete.value = false;
  } catch (e) {
    errMsg.value = String(e).replace(/^.*Error: /, "");
  } finally {
    deleteBusy.value = false;
  }
}
</script>

<template>
  <div class="data">
    <div class="block">
      <p class="t">导出全部数据（JSON）</p>
      <p class="d">把应用、区间、汇总、任务、提醒规则一次性导出为 JSON，位置自选（可用于换机恢复）</p>
      <NButton size="small" secondary :loading="busy === 'export'" @click="exportJson">
        选择位置并导出
      </NButton>
    </div>

    <div class="block">
      <p class="t">导出 Tai 格式</p>
      <p class="d">
        生成一个 Tai 能直接使用的 data.db（放进 Tai 的 Data 目录即可）+ 与 Tai 导出文件同列结构的
        每日/时段 CSV，共 4 个文件（data.db + xlsx 表格 + 2 个 CSV）
      </p>
      <NButton size="small" secondary :loading="busy === 'tai'" @click="exportTai">
        选择位置并导出
      </NButton>
    </div>

    <div class="block">
      <p class="t">从导出文件恢复</p>
      <p class="d">⚠ 仅当当前数据库为空时可用（换机/重装后的全新恢复），避免数据叠加</p>
      <NButton size="small" secondary :loading="busy === 'restore'" @click="restoreJson">
        选择文件并恢复
      </NButton>
    </div>

    <div class="block">
      <p class="t danger">删除时间记录数据</p>
      <p class="d">删除后无法恢复（建议先导出备份）。任务与提醒规则不受影响</p>
      <div class="line">
        <NSelect
          v-model:value="deleteScope"
          size="small"
          :options="deleteScopeOptions"
          style="width: 300px"
        />
        <NButton size="small" type="error" secondary @click="showDelete = true">删除…</NButton>
      </div>
    </div>

    <p v-if="okMsg" class="ok pre">{{ okMsg }}</p>
    <p v-if="errMsg" class="err">{{ errMsg }}</p>

    <NModal
      v-model:show="showDelete"
      preset="dialog"
      type="warning"
      title="确认删除？"
      positive-text="确认删除"
      negative-text="先不删"
      :loading="deleteBusy"
      @positive-click="doDelete"
    >
      <p>
        即将删除：<b>{{ deleteScope === "today" ? "今天" : "全部" }}</b
      >的时间记录数据（区间、汇总、键鼠计数{{ deleteScope === "all" ? "、应用列表" : "" }}）。
      </p>
      <p>此操作不可撤销。确定继续吗？</p>
    </NModal>
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
  color: var(--text);
}

.block .t.danger {
  color: var(--danger);
}

.block .d {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--text-muted);
}

.line {
  display: flex;
  gap: 8px;
  align-items: center;
}

.ok {
  margin: 0;
  font-size: 12px;
  color: var(--good);
  word-break: break-all;
  white-space: pre-line;
}

.err {
  margin: 0;
  font-size: 12px;
  color: var(--danger);
}
</style>
