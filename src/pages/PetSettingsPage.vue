<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { NButton, NPopconfirm, NSlider, NSwitch } from "naive-ui";
import type { PetSettings } from "../types";
import Icon from "../components/Icon.vue";
import DeleteButton from "../components/DeleteButton.vue";
import BallLoader from "../components/BallLoader.vue";

interface ModelInfo {
  id: string;
  name: string;
  defaultName: string;
  mode: string;
  builtin: boolean;
  active: boolean;
  renamed: boolean;
  live2d: boolean;
}

interface ImportOutcome {
  id: string;
  name: string;
  mode: string;
  live2d: boolean;
  deduped: boolean;
  suffixed: boolean;
}

defineProps<{ petVisible: boolean }>();
const emit = defineEmits<{
  (e: "update:petVisible", v: boolean): void;
  (e: "togglePet"): void;
}>();

const settings = ref<PetSettings | null>(null);
const models = ref<ModelInfo[]>([]);
const importing = ref(false);
const msg = ref("");
const errMsg = ref("");

/* 滑条：本地即时值 → 松手落盘
   注意：naive-ui 的 Slider 只发 update:value / dragend，没有 change 事件
   （上一版绑 change，因此拖动从来没有真正写回过后端） */
const localScale = ref(100);
const localOpacity = ref(100);
let scaleTimer: number | undefined;
let opacityTimer: number | undefined;

const modeLabel: Record<string, string> = {
  keyboard: "双爪键盘",
  gamepad: "手柄",
  standard: "单手",
};

async function load() {
  try {
    settings.value = await invoke<PetSettings>("pet_settings_get");
    localScale.value = settings.value.scale;
    localOpacity.value = settings.value.opacity;
    models.value = await invoke("pet_models_list");
  } catch (e) {
    errMsg.value = String(e).replace(/^.*Error: /, "");
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

function onScaleInput(v: number) {
  localScale.value = v;
  if (scaleTimer) clearTimeout(scaleTimer);
  scaleTimer = window.setTimeout(() => void applyWindow({ scale: localScale.value }), 200);
}

function onOpacityInput(v: number) {
  localOpacity.value = v;
  if (opacityTimer) clearTimeout(opacityTimer);
  opacityTimer = window.setTimeout(() => void applyWindow({ opacity: localOpacity.value }), 200);
}

function commitScale() {
  if (scaleTimer) clearTimeout(scaleTimer);
  void applyWindow({ scale: localScale.value });
}

function commitOpacity() {
  if (opacityTimer) clearTimeout(opacityTimer);
  void applyWindow({ opacity: localOpacity.value });
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
    errMsg.value = String(e).replace(/^.*Error: /, "");
  }
}

/* ---------- 模型重命名（只改显示名，不动磁盘目录） ---------- */
const editingId = ref("");
const editName = ref("");

function startRename(m: ModelInfo) {
  editingId.value = m.id;
  editName.value = m.name;
  msg.value = "";
  errMsg.value = "";
}

async function commitRename() {
  const id = editingId.value;
  if (!id) return;
  try {
    models.value = await invoke("pet_model_rename", { id, name: editName.value });
    msg.value = "已更新模型名";
    editingId.value = "";
  } catch (e) {
    errMsg.value = String(e).replace(/^.*Error: /, "");
  }
}

/* ---------- 导入 ---------- */
async function importFolder() {
  await doImport(
    await open({ multiple: false, directory: true, title: "选择模型文件夹（含 img 与 config.json）" })
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
    const info = await invoke<ImportOutcome>("pet_import_model", { path: picked });
    if (info.deduped) {
      msg.value = `这个模型已经导入过了（「${info.name}」），未重复导入`;
    } else if (info.suffixed) {
      msg.value = `已导入为「${info.name}」（同名模型已存在，自动加了编号）`;
    } else {
      msg.value = `已导入模型「${info.name}」，点「使用」启用`;
    }
    if (info.live2d) {
      msg.value += "。注意：这个模型含 Live2D 素材，静态图只是兜底，需要 Live2D 运行时才是它本来的样子。";
    }
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
      <span class="sub">显示开关、模型与窗口外观</span>
    </header>

    <!-- 显示开关（从个性化页移到这里） -->
    <section class="glass-card">
      <h2>显示</h2>
      <div class="row">
        <div class="rlabel">
          <p class="rt">显示桌宠</p>
          <p class="rd">屏幕右下角的猫，托盘右键菜单也能开关</p>
        </div>
        <NSwitch :value="petVisible" @update:value="(v: boolean) => { emit('update:petVisible', v); emit('togglePet'); }" />
      </div>
    </section>

    <!-- 模型管理 -->
    <section class="glass-card">
      <h2>模型</h2>
      <div class="mlist">
        <div v-for="m in models" :key="m.id" class="mrow" :class="{ active: m.active }">
          <span class="mdot"></span>
          <template v-if="editingId === m.id">
            <input
              v-model="editName"
              class="medit"
              maxlength="40"
              @keyup.enter="commitRename"
              @keyup.esc="editingId = ''"
            />
            <NButton size="tiny" type="primary" secondary @click="commitRename">保存</NButton>
            <NButton size="tiny" quaternary @click="editingId = ''">取消</NButton>
          </template>
          <template v-else>
            <span class="mname" :title="m.name" @click="startRename(m)">{{ m.name }}</span>
            <span class="mtag">{{ modeLabel[m.mode] ?? m.mode }}</span>
            <span v-if="m.live2d" class="mtag live2d" title="含 Live2D 素材：静态图只是兜底，需要 Live2D 运行时">
              Live2D
            </span>
            <span v-if="m.builtin" class="mtag builtin">内置</span>
            <span v-if="m.active" class="mtag using">使用中</span>
            <div class="macts">
              <NButton size="tiny" quaternary @click="startRename(m)">改名</NButton>
              <NButton v-if="!m.active" size="tiny" type="primary" secondary @click="setActive(m.id)">
                使用
              </NButton>
              <NPopconfirm v-if="!m.builtin" @positive-click="removeModel(m.id)">
                <template #trigger>
                  <DeleteButton size="sm" />
                </template>
                确定删除模型「{{ m.name }}」吗？磁盘上的模型文件夹也会一起删掉。
              </NPopconfirm>
            </div>
          </template>
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
        <span v-if="!importing" class="ihint">支持 Mver 模型包（img/ + config.json 或 bongocat.skin.json）</span>
      </div>
      <BallLoader v-if="importing" label="正在导入模型…" />
      <p class="rd more">
        点模型名或「改名」可以自由命名（只改显示名，不动磁盘目录）；同一个模型重复导入会被自动识别，不会装两遍。
      </p>
      <p class="rd more">
        关于「兔子洞」：它的美术只有 Live2D 一份（<code>Bunny.moc3</code>），
        分层 PNG 里没有兔子洞本体，所以现在只能显示原版 BongoCat 的静态图。
        要真正显示兔子洞需要 Live2D 运行时（Cubism Core + 渲染库），属于要单独征求你同意的依赖。
      </p>
    </section>

    <!-- 窗口外观 -->
    <section class="glass-card">
      <h2>窗口外观</h2>
      <div class="row col">
        <div class="rlabel">
          <p class="rt">缩放</p>
          <p class="rd">50% ~ 200%</p>
        </div>
        <NSlider
          :value="localScale"
          :min="50"
          :max="200"
          :step="5"
          :format-tooltip="(v: number) => v + '%'"
          style="max-width: 320px"
          @update:value="onScaleInput"
          @dragend="commitScale"
        />
      </div>
      <div class="row col">
        <div class="rlabel">
          <p class="rt">不透明度</p>
          <p class="rd">30% ~ 100%</p>
        </div>
        <NSlider
          :value="localOpacity"
          :min="30"
          :max="100"
          :step="5"
          :format-tooltip="(v: number) => v + '%'"
          style="max-width: 320px"
          @update:value="onOpacityInput"
          @dragend="commitOpacity"
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
  cursor: text;
}

.medit {
  flex: 1;
  min-width: 0;
  border: 1px solid var(--accent-border);
  background: var(--surface-solid);
  color: var(--text);
  border-radius: var(--r-sm);
  font-size: 13px;
  font-family: inherit;
  padding: 5px 8px;
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

.mtag.live2d {
  color: var(--warn);
  background: var(--warn-soft);
}

.rd.more code {
  font-family: Consolas, monospace;
  font-size: 11px;
  color: var(--text-muted);
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

.rd.more {
  margin: 8px 0 0;
  font-size: 11.5px;
  line-height: 1.7;
  color: var(--text-faint);
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
