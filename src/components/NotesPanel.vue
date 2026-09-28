<script setup lang="ts">
import { onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { NButton, NDatePicker, NInput } from "naive-ui";

/** 每日日志：Markdown 正文 + 图片附件（后端 notes.rs） */
const props = defineProps<{ dayTs?: number }>();
interface DailyNote {
  date: string;
  content: string;
  images: string[];
  updatedTs: number;
}

const day = ref(Date.now());
const content = ref("");
const images = ref<string[]>([]);
const thumbs = reactive<Record<string, string>>({});
const recent = ref<DailyNote[]>([]);
const msg = ref("");
const err = ref("");
const busy = ref(false);

function ymd(ts: number): string {
  const d = new Date(ts);
  return `${d.getFullYear()}-${`${d.getMonth() + 1}`.padStart(2, "0")}-${`${d.getDate()}`.padStart(2, "0")}`;
}

const today = ymd(Date.now());
const dateStr = () => ymd(day.value);

/** 附件加载为 data URL（同一天内缓存） */
async function ensureThumb(date: string, name: string) {
  const key = `${date}/${name}`;
  if (thumbs[key]) return;
  try {
    const [mime, b64] = await invoke<[string, string]>("notes_image_data", { date, name });
    thumbs[key] = `data:${mime};base64,${b64}`;
  } catch {
    /* 忽略单张读失败 */
  }
}

async function loadList() {
  try {
    recent.value = await invoke<DailyNote[]>("notes_recent", { limit: 30 });
  } catch {
    recent.value = [];
  }
}

async function loadDay() {
  err.value = "";
  msg.value = "";
  const date = dateStr();
  try {
    const n = await invoke<DailyNote | null>("notes_get", { date });
    content.value = n?.content ?? "";
    images.value = n?.images ?? [];
    for (const name of images.value) void ensureThumb(date, name);
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function saveDay() {
  busy.value = true;
  err.value = "";
  msg.value = "";
  try {
    await invoke("notes_save", { date: dateStr(), content: content.value, images: images.value });
    msg.value = `已保存 ${dateStr()} 的日志`;
    await loadList();
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  } finally {
    busy.value = false;
  }
}

async function addImage() {
  err.value = "";
  try {
    const picked = await open({
      multiple: true,
      directory: false,
      title: "选择图片",
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
    });
    if (!picked) return;
    const list = Array.isArray(picked) ? picked : [picked];
    for (const p of list) {
      const name = await invoke<string>("notes_add_image", { date: dateStr(), path: p });
      images.value.push(name);
      void ensureThumb(dateStr(), name);
    }
    msg.value = `已添加 ${list.length} 张图片（点保存后写入当天日志）`;
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

function removeImage(name: string) {
  images.value = images.value.filter((x) => x !== name);
  msg.value = "已从这天移除（原图仍留在数据目录里，未删除）";
}

async function exportMd() {
  err.value = "";
  try {
    const dir = await open({ directory: true, multiple: false, title: "选择导出目录" });
    if (!dir || Array.isArray(dir)) return;
    const n = await invoke<number>("notes_export_md", { dir });
    msg.value = `已导出 ${n} 篇日志（含 images 图片目录）到：${dir}`;
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function importMd() {
  err.value = "";
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择要导入的 Markdown 日志",
      filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    const [, replaced] = await invoke<[number, number]>("notes_import_md", { path: picked });
    await loadList();
    await loadDay();
    msg.value = replaced > 0 ? "已导入并覆盖同日期日志" : "已导入日志";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function pickRecent(d: string) {
  day.value = new Date(`${d}T00:00:00`).getTime();
  await loadDay();
}

// 从月历点一天过来：切到那天
watch(
  () => props.dayTs,
  async (v) => {
    if (!v) return;
    day.value = v;
    await loadDay();
  }
);

onMounted(async () => {
  await loadList();
  await loadDay();
});
</script>

<template>
  <div class="notes">
    <div class="bar">
      <NDatePicker
        v-model:value="day"
        type="date"
        :actions="['confirm']"
        :clearable="false"
        style="width: 170px"
        @update:value="loadDay"
      />
      <span class="d">{{ dateStr() }}{{ dateStr() === today ? " · 今天" : "" }}</span>
      <div class="acts">
        <NButton size="tiny" quaternary @click="addImage">添加图片</NButton>
        <NButton size="tiny" quaternary @click="importMd">导入 MD</NButton>
        <NButton size="tiny" quaternary @click="exportMd">导出全部 MD</NButton>
        <NButton size="small" type="primary" secondary :loading="busy" @click="saveDay">保存</NButton>
      </div>
    </div>

    <div class="chips">
      <button
        v-for="n in recent"
        :key="n.date"
        class="chip"
        :class="{ active: n.date === dateStr() }"
        @click="pickRecent(n.date)"
      >
        <span>{{ n.date.slice(5) }}</span>
        <span class="cnt">{{ n.images.length ? `${n.images.length}图` : "" }}</span>
      </button>
      <span v-if="!recent.length" class="empty">还没有日志，写第一条试试</span>
    </div>

    <NInput
      v-model:value="content"
      type="textarea"
      placeholder="写点什么…支持 Markdown（# 标题、- 列表、**加粗**、`代码`）"
      :autosize="{ minRows: 6, maxRows: 18 }"
    />

    <div v-if="images.length" class="imgs">
      <div v-for="name in images" :key="name" class="img">
        <img v-if="thumbs[`${dateStr()}/${name}`]" :src="thumbs[`${dateStr()}/${name}`]" alt="" />
        <span v-else class="none">读取中…</span>
        <button class="rm" title="从此天移除" @click="removeImage(name)">×</button>
      </div>
    </div>

    <p v-if="msg" class="ok">{{ msg }}</p>
    <p v-if="err" class="err">{{ err }}</p>
  </div>
</template>

<style scoped>
.notes {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.bar {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.d {
  font-size: 12px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.acts {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 4px;
}

.chips {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-full);
  font-size: 11px;
  font-family: inherit;
  padding: 3px 10px;
  cursor: pointer;
  transition: background var(--dur), color var(--dur), transform var(--dur);
}

.chip:hover {
  transform: translateY(calc(-1px * var(--motion)));
  color: var(--text);
}

.chip.active {
  color: var(--accent-text);
  background: var(--accent-soft);
  border-color: var(--accent-border);
}

.cnt {
  color: var(--text-faint);
  font-size: 10px;
}

.imgs {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.img {
  position: relative;
  width: 96px;
  height: 72px;
  border-radius: var(--r-sm);
  overflow: hidden;
  border: 1px solid var(--border);
  background: var(--surface);
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.img img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.none {
  font-size: 10px;
  color: var(--text-faint);
}

.rm {
  position: absolute;
  top: 2px;
  right: 2px;
  width: 18px;
  height: 18px;
  border: 0;
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 12px;
  line-height: 1;
  cursor: pointer;
}

.empty {
  font-size: 12px;
  color: var(--text-faint);
}

.ok {
  margin: 0;
  font-size: 12px;
  color: var(--good);
  word-break: break-all;
}

.err {
  margin: 0;
  font-size: 12px;
  color: var(--danger);
}
</style>
