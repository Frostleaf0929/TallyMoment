<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { NButton, NDatePicker, NInput } from "naive-ui";
import MarkdownPreview from "./MarkdownPreview.vue";
import { appendBlock } from "../lib/mdBlocks";

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
/** 预览（块式），插入的图片宽度 */
const showPreview = ref(true);
const imgWidth = ref<"100%" | "60%" | "33%">("100%");

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

/* ---------- 块工具栏：把内容按"块"追加，再用预览里的 ↑↓ 调整顺序 ---------- */
function insertBlock(raw: string) {
  content.value = appendBlock(content.value, raw);
}

/** 插入图片块：按所选宽度写成 <img>，全宽时用标准 ![]() 写法（Obsidian 兼容） */
function insertImageBlock(name: string) {
  const raw =
    imgWidth.value === "100%"
      ? `![](images/${name})`
      : `<img src="images/${name}" width="${imgWidth.value}">`;
  insertBlock(raw);
}

/** 缩略图映射：给预览渲染用 */
const imageMap = computed(() => {
  const m: Record<string, string> = {};
  for (const name of images.value) {
    const url = thumbs[`${dateStr()}/${name}`];
    if (url) m[name] = url;
  }
  return m;
});

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
      await ensureThumb(dateStr(), name);
      insertImageBlock(name);
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

    <!-- 块工具栏：点一下就往正文追加一个"块"，顺序可在下面预览里用 ↑↓ 调整 -->
    <div class="blocks">
      <button class="tb" @click="insertBlock('# ' + '标题')">标题</button>
      <button class="tb" @click="insertBlock('正文')">正文</button>
      <button class="tb" @click="insertBlock('- [ ] ' + '待办')">待办</button>
      <button class="tb" @click="insertBlock('- ' + '列表项')">列表</button>
      <button class="tb" @click="insertBlock('> ' + '引用')">引用</button>
      <button class="tb" @click="insertBlock('---')">分割线</button>
      <button class="tb" @click="insertBlock('**加粗文字**')">加粗</button>
      <span class="tb-sep"></span>
      <button class="tb" @click="addImage">图片…</button>
      <span class="tb-w">
        宽度
        <button class="tb sm" :class="{ active: imgWidth === '33%' }" @click="imgWidth = '33%'">小</button>
        <button class="tb sm" :class="{ active: imgWidth === '60%' }" @click="imgWidth = '60%'">中</button>
        <button class="tb sm" :class="{ active: imgWidth === '100%' }" @click="imgWidth = '100%'">全宽</button>
      </span>
      <button class="tb" style="margin-left: auto" @click="showPreview = !showPreview">
        {{ showPreview ? "隐藏预览" : "显示预览" }}
      </button>
    </div>

    <NInput
      v-model:value="content"
      type="textarea"
      placeholder="写点什么…也可以直接手写 Markdown"
      :autosize="{ minRows: 5, maxRows: 14 }"
    />

    <div v-if="showPreview" class="preview">
      <MarkdownPreview
        :content="content"
        :images="imageMap"
        @update:content="(v: string) => (content = v)"
      />
    </div>

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

.blocks {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}

.tb {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-sm);
  font-size: 11.5px;
  font-family: inherit;
  padding: 3px 9px;
  cursor: pointer;
  transition: background var(--dur), color var(--dur), border-color var(--dur);
}

.tb:hover {
  background: var(--surface-hover);
  color: var(--text);
}

.tb.sm {
  padding: 2px 7px;
  font-size: 11px;
}

.tb.sm.active {
  color: var(--accent-text);
  background: var(--accent-soft);
  border-color: var(--accent-border);
}

.tb-sep {
  width: 1px;
  height: 16px;
  background: var(--border);
  margin: 0 2px;
}

.tb-w {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 11px;
  color: var(--text-faint);
}

.preview {
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  background: var(--surface);
  padding: 8px 10px;
  max-height: 320px;
  overflow-y: auto;
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
