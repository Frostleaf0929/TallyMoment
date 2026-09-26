<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { NButton, NPopconfirm, NSlider, NSwitch } from "naive-ui";
import type { PetSettings } from "../types";
import Icon from "../components/Icon.vue";

const settings = ref<PetSettings | null>(null);
const models = ref<{ id: string; name: string; mode: string; builtin: boolean; active: boolean }[]>([]);
const importing = ref(false);
const msg = ref("");
const errMsg = ref("");

// 滑块拖动中先存本地，松手后提交
const pendingScale = ref<number | null>(null);
const pendingOpacity = ref<number | null>(null);

async function load() {
  try {
    settings.value = await invoke<PetSettings>("pet_settings_get");
    models.value = await invoke("pet_models_list");
  } catch {
    /* 忽略 */
  }
}

async function applyWindow(opts: { scale?: number; opacity?: number }) {
  const s = settings.value;
  if (!s) return;
  try {
    settings.value = await invoke<PetSettings>("pet_settings_set", {
      scale: opts.scale ?? s.scale,
      opacity: opts.opacity ?? s.opacity,
      alwaysOnTop: s.alwaysOnTop,
      passThrough: s.passThrough,
      mirror: s.mirror,
    });
    errMsg.value = "";
  } catch (e) {
    errMsg.value = String(e).replace(/^.*Error: /, "");
  }
}

function onScale(v: number) {
  pendingScale.value = v;
  if (pendingOpacity.value === null) pendingOpacity.value = settings.value?.opacity ?? 100;
}

function onOpacity(v: number) {
  pendingOpacity.value = v;
  if (pendingScale.value === null) pendingScale.value = settings.value?.scale ?? 100;
}

function commitSliders() {
  const s = pendingScale.value;
  const o = pendingOpacity.value;
  if (s !== null && o !== null) void applyWindow({ scale: s, opacity: o });
  pendingScale.value = null;
  pendingOpacity.value = null;
}

async function toggleAlwaysTop(v: boolean) {
  if (!settings.value) return;
  settings.value.alwaysOnTop = v;
  await applyWindow({});
}

async function togglePassThrough(v: boolean) {
  if (!settings.value) return;
  settings.value.passThrough = v;
  await applyWindow({});
}

async function toggleMirror(v: boolean) {
  if (!settings.value) return;
  settings.value.mirror = v;
  await applyWindow({});
}

async function resetPosition() {
  msg.value = "";
  try {
    await invoke("pet_reset_position");
    msg.value = "桌宠已回到默认位置";
  } catch (e) {
    errMsg.value = String(e);
  }
}

async function importFolder() {
  await doImport(
    await open({ multiple: false, directory: true, title: "选择模型文件夹（含 config.json）" })
  );
}

async function importZip() {
  await doImport(
    await open({
      multiple: false,
      directory: false,
      title: "选择模型 ZIP",
      filters: [{ name: "模型压缩包", extensions: ["zip"] }],
    })
  );
}

async function doImport(picked: string | null) {
  if (!picked) return;
  importing.value = true;
  msg.value = "";
  errMsg.value = "";
  try {
    const info = await invoke<{ id: string; name: string }>("pet_import_model", { path: picked });
    msg.value = `已导入模型「${info.name}」，点“使用”启用`;
    await load();
  } catch (e) {
    errMsg.value = String(e).replace(/^.*Error: /, "");
  } finally {
    importing.value = false;
  }
}

async function setActive(id: string) {
  errMsg.value = "";
  try {
    settings.value = await invoke<PetSettings>("pet_model_set_active", { id });
    await load();
    msg.value = "模型已切换";
  } catch (e) {
    errMsg.value = String(e).replace(/^.*Error: /, "");
  }
}

async function removeModel(id: string) {
  errMsg.value = "";
  try {
    models.value = await invoke("pet_model_delete", { id });
    if (settings.value?.activeModel === id) {
      settings.value = await invoke<PetSettings>("pet_settings_get");
    }
    msg.value = "模型已删除";
  } catch (e) {
    errMsg.value = String(e).replace(/^.*Error: /, "");
  }
}

onMounted(load);
</script>

<template>
  <div class="petsettings">
    <header class="phead">
      <h1>桌宠</h1>
      <span class="sub">模型、外观与窗口行为（对标新版 BongoCat 的设置子集）</span>
    </header>

    <!-- 模型管理 -->
    <section class="glass-card">
      <h2>模型</h2>
      <div class="mlist">
        <div v-for="m in models" :key="m.id" class="mrow" :class="{ active: m.active }">
          <span class="mdot"></span>
          <span class="mname">{{ m.name }}</span>
          <span class="mtag">{{ m.mode === "keyboard" ? "双爪键盘" : "单手" }}</span>
          <span v-if="m.builtin" class="mtag builtin">内置</span>
          <span v-if="m.active" class="mtag using">使用中</span>
          <div class="macts">
            <NButton v-if="!m.active" size="tiny" type="primary" secondary @click="setActive(m.id)">
              使用
            </NButton>
            <NPopconfirm v-if="!m.builtin" @positive-click="removeModel(m.id)">
              <template #trigger>
                <NButton quaternary size="tiny" type="error">删除</NButton>
              </template>
              删除模型「{{ m.name }}」？
            </NPopconfirm>
          </div>
        </div>
        <p v-if="!models.length" class="empty">暂无模型</p>
      </div>
      <div class="imports">
        <NButton size="small" secondary :loading="importing" @click="importFolder">
          <Icon name="plus" :size="14" /> 导入文件夹
        </NButton>
        <NButton size="small" secondary :loading="importing" @click="importZip">
          <Icon name="doc" :size="14" /> 导入 ZIP
        </NButton>
        <span class="ihint">支持 Mver 模型包（含 config.json + img/），按键映射自动生效</span>
      </div>
    </section>

    <!-- 窗口外观 -->
    <section class="glass-card">
      <h2>窗口外观</h2>
      <div class="row col">
        <div class="rlabel">
          <p class="rt">缩放</p>
          <p class="rd">30% ~ 200%</p>
        </div>
        <NSlider
          :value="settings?.scale ?? 100"
          :min="50"
          :max="200"
          :step="5"
          :format-tooltip="(v: number) => v + '%'"
          style="max-width: 320px"
          @update:value="onScale"
          @change="commitSliders"
        />
      </div>
      <div class="row col">
        <div class="rlabel">
          <p class="rt">不透明度</p>
          <p class="rd">30% ~ 100%</p>
        </div>
        <NSlider
          :value="settings?.opacity ?? 100"
          :min="30"
          :max="100"
          :step="5"
          :format-tooltip="(v: number) => v + '%'"
          style="max-width: 320px"
          @update:value="onOpacity"
          @change="commitSliders"
        />
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">窗口置顶</p>
          <p class="rd">桌宠始终显示在其他窗口之上</p>
        </div>
        <NSwitch :value="settings?.alwaysOnTop ?? true" @update:value="toggleAlwaysTop" />
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">鼠标点击穿透</p>
          <p class="rd">开启后桌宠不拦截任何鼠标操作（关闭需回此页或重启）</p>
        </div>
        <NSwitch :value="settings?.passThrough ?? false" @update:value="togglePassThrough" />
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">镜像翻转</p>
          <p class="rd">水平翻转桌宠朝向</p>
        </div>
        <NSwitch :value="settings?.mirror ?? false" @update:value="toggleMirror" />
      </div>
      <div class="row">
        <div class="rlabel">
          <p class="rt">位置</p>
          <p class="rd">拖动桌宠可自由摆放；拖不动时先关闭点击穿透</p>
        </div>
        <NButton size="small" secondary @click="resetPosition">重置位置</NButton>
      </div>
    </section>

    <p v-if="msg" class="ok">{{ msg }}</p>
    <p v-if="errMsg" class="err">{{ errMsg }}</p>
  </div>
</template>

<style scoped>
.petsettings {
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

.glass-card {
  padding: 16px 18px;
}

.glass-card h2 {
  margin: 0 0 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

.mlist {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 12px;
}

.mrow {
  display: flex;
  align-items: center;
  gap: 10px;
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-md);
  padding: 9px 12px;
}

.mrow.active {
  border-color: var(--accent-border);
  background: var(--accent-soft);
}

.mdot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--accent);
  flex: none;
}

.mname {
  flex: 1;
  font-size: 13px;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mtag {
  font-size: 10px;
  color: var(--text-muted);
  background: var(--surface-hover);
  border-radius: var(--r-full);
  padding: 2px 9px;
}

.mtag.builtin,
.mtag.using {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.macts {
  display: flex;
  align-items: center;
  gap: 6px;
}

.imports {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.ihint {
  font-size: 11px;
  color: var(--text-faint);
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

.ok {
  margin: 0;
  font-size: 12px;
  color: var(--good);
}

.err {
  margin: 0;
  font-size: 12px;
  color: var(--danger);
}
</style>
