<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NInput, NPopconfirm, NSwitch } from "naive-ui";

interface Item {
  id: number;
  name: string;
  startTime: string;
  endTime: string;
  remindStart: boolean;
  remindEnd: boolean;
  enabled: boolean;
}

const items = ref<Item[]>([]);
const name = ref("");
const startT = ref("");
const endT = ref("");
const error = ref("");

async function load() {
  try {
    items.value = await invoke<Item[]>("checklist_list");
  } catch {
    /* 忽略：主界面早于后端就绪 */
  }
}

async function add() {
  error.value = "";
  if (!name.value.trim() || !startT.value || !endT.value) {
    error.value = "请填写名称和起止时间";
    return;
  }
  try {
    await invoke("checklist_add", {
      name: name.value,
      startTime: startT.value,
      endTime: endT.value,
      remindStart: true,
      remindEnd: true,
    });
    name.value = "";
    startT.value = "";
    endT.value = "";
    await load();
  } catch (e) {
    error.value = String(e).replace(/^.*Error: /, "");
  }
}

async function toggle(item: Item, v: boolean) {
  try {
    await invoke("checklist_set_enabled", { id: item.id, enabled: v });
    item.enabled = v;
  } catch {
    item.enabled = !v;
  }
}

async function remove(item: Item) {
  await invoke("checklist_delete", { id: item.id });
  await load();
}

onMounted(load);
</script>

<template>
  <div class="wrap">
    <div v-for="it in items" :key="it.id" class="row" :class="{ off: !it.enabled }">
      <span class="time">{{ it.startTime }}–{{ it.endTime }}</span>
      <span class="name" :title="it.name">{{ it.name }}</span>
      <NSwitch
        size="small"
        :value="it.enabled"
        @update:value="(v: boolean) => toggle(it, v)"
      />
      <NPopconfirm @positive-click="remove(it)">
        <template #trigger>
          <NButton quaternary size="tiny" type="error">删除</NButton>
        </template>
        确定删除「{{ it.name }}」？
      </NPopconfirm>
    </div>

    <p v-if="!items.length" class="empty">还没有清单项，加一条试试 ↓</p>

    <div class="add">
      <NInput
        v-model:value="name"
        size="small"
        placeholder="清单项名称，如：晚间学习"
        maxlength="50"
        @keyup.enter="add"
      />
      <input v-model="startT" class="tp" type="time" />
      <span class="sep">–</span>
      <input v-model="endT" class="tp" type="time" />
      <NButton size="small" type="primary" secondary @click="add">添加</NButton>
    </div>
    <p v-if="error" class="err">{{ error }}</p>
  </div>
</template>

<style scoped>
.wrap {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 10px;
  border: 1px solid #232936;
  background: #12151f;
  border-radius: 8px;
}

.row.off .name,
.row.off .time {
  opacity: 0.45;
}

.time {
  font-size: 12px;
  color: #a5b4fc;
  font-variant-numeric: tabular-nums;
  min-width: 92px;
}

.name {
  flex: 1;
  font-size: 13px;
  color: #d3d8e4;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.empty {
  margin: 0;
  font-size: 12px;
  color: #5c6474;
}

.add {
  display: flex;
  align-items: center;
  gap: 8px;
}

.add > :first-child {
  flex: 1;
}

.tp {
  border: 1px solid #2a2f3d;
  background: #161a24;
  color: #d3d8e4;
  border-radius: 6px;
  padding: 4px 6px;
  font-size: 12px;
  font-family: inherit;
  outline: none;
}

.sep {
  color: #5c6474;
}

.err {
  margin: 0;
  font-size: 12px;
  color: #f87171;
}
</style>
