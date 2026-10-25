<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { NButton, NDatePicker, NInput } from "naive-ui";
import MarkdownPreview from "./MarkdownPreview.vue";
import {
  appendBlock,
  insertAtCursor,
  toggleLinePrefix,
  wrapSelection,
  type SelEdit,
} from "../lib/mdBlocks";

/** 每日日志：默认是记录列表（列表 / 画廊两种看法），点开某天才进编辑器（与任务/提醒同一套交互） */
const props = withDefaults(defineProps<{ dayTs?: number; defaultView?: "list" | "gallery" }>(), {
  defaultView: "list",
});
interface DailyNote {
  date: string;
  content: string;
  images: string[];
  updatedTs: number;
}

const day = ref(Date.now());
const editorOpen = ref(false);
const content = ref("");
const images = ref<string[]>([]);
const thumbs = reactive<Record<string, string>>({});
const msg = ref("");
const err = ref("");
const busy = ref(false);
/** 视图三态：edit 只编辑 / split 上下分栏（编辑+预览）/ preview 只读预览 */
/** 编辑器视图：记忆上次选择（首次默认编辑） */
const noteView = ref<"edit" | "split" | "preview">(
  (localStorage.getItem("notes.view") as "edit" | "split" | "preview") || "edit"
);
watch(noteView, (v) => {
  try {
    localStorage.setItem("notes.view", v);
  } catch {
    /* 忽略 */
  }
});
/** 插入图片的宽度（新图片块用） */
const imgWidth = ref<"100%" | "60%" | "33%">("100%");

/* ---------- 记录列表（默认主体） ---------- */
const allNotes = ref<DailyNote[]>([]);
const listLoaded = ref(false);
const searchText = ref("");
/** 列表 / 画廊：同样记忆上次选择（没记过才用调用方给的默认值） */
const listMode = ref<"list" | "gallery">(
  (localStorage.getItem("notes.listMode") as "list" | "gallery") || props.defaultView
);
watch(listMode, (v) => {
  try {
    localStorage.setItem("notes.listMode", v);
  } catch {
    /* 忽略 */
  }
});

function ymd(ts: number): string {
  const d = new Date(ts);
  return `${d.getFullYear()}-${`${d.getMonth() + 1}`.padStart(2, "0")}-${`${d.getDate()}`.padStart(2, "0")}`;
}

const today = ymd(Date.now());
const dateStr = () => ymd(day.value);

async function loadList() {
  try {
    allNotes.value = await invoke<DailyNote[]>("notes_recent", { limit: 365 });
    listLoaded.value = true;
  } catch {
    allNotes.value = [];
  }
}

/** 摘要：剥掉 Markdown 标记与图片语法，取开头一段 */
function brief(text: string): string {
  return text
    .replace(/<img[^>]*>/g, " ")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/[*_~`>|-]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

function firstLine(text: string): string {
  for (const raw of text.split("\n")) {
    const l = brief(raw);
    if (l) return l.slice(0, 40);
  }
  return "";
}

const filteredNotes = computed(() => {
  const q = searchText.value.trim().toLowerCase();
  if (!q) return allNotes.value;
  return allNotes.value.filter(
    (n) => n.date.includes(q) || n.content.toLowerCase().includes(q)
  );
});

/* ---------- 画廊封面：懒加载（每篇取第一张图，缩到 320px） ---------- */
const covers = reactive<Record<string, string>>({});
let coversStarted = false;

function toThumb(dataUrl: string): Promise<string> {
  return new Promise((resolve) => {
    const img = new Image();
    img.decoding = "async";
    img.onload = () => {
      try {
        const k = Math.min(1, 320 / Math.max(img.width, img.height));
        const cv = document.createElement("canvas");
        cv.width = Math.max(1, Math.round(img.width * k));
        cv.height = Math.max(1, Math.round(img.height * k));
        const ctx = cv.getContext("2d");
        if (!ctx) return resolve(dataUrl);
        ctx.drawImage(img, 0, 0, cv.width, cv.height);
        resolve(cv.toDataURL("image/jpeg", 0.8));
      } catch {
        resolve(dataUrl);
      }
    };
    img.onerror = () => resolve(dataUrl);
    img.src = dataUrl;
  });
}

async function loadCovers() {
  if (coversStarted) return;
  coversStarted = true;
  let n = 0;
  for (const note of allNotes.value) {
    if (n >= 60) break; // 封面懒加载上限：最多 60 篇，其余显示文字卡
    if (!note.images.length || covers[note.date]) continue;
    n++;
    try {
      const [mime, b64] = await invoke<[string, string]>("notes_image_data", {
        date: note.date,
        name: note.images[0],
      });
      covers[note.date] = await toThumb(`data:${mime};base64,${b64}`);
    } catch {
      /* 单篇失败就显示文字卡 */
    }
  }
}

watch(listMode, (m) => {
  if (m === "gallery") void loadCovers();
});

/* ---------- 编辑器（次级视图） ---------- */
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

async function loadDay() {
  err.value = "";
  msg.value = "";
  const date = dateStr();
  try {
    const n = await invoke<DailyNote | null>("notes_get", { date });
    content.value = n?.content ?? "";
    images.value = n?.images ?? [];
    for (const name of images.value) void ensureThumb(date, name);
    // 本地草稿：上次没保存就离开的内容，提示恢复（否则清掉）
    draftRestore.value = null;
    try {
      const raw = localStorage.getItem(draftKey(date));
      if (raw) {
        const d = JSON.parse(raw) as { content?: string; images?: string[]; at?: number };
        if (typeof d.content === "string" && d.content !== content.value) {
          draftRestore.value = { content: d.content, images: Array.isArray(d.images) ? d.images : null, at: d.at ?? 0 };
          msg.value = "检测到未保存的草稿";
        } else {
          localStorage.removeItem(draftKey(date));
        }
      }
    } catch {
      /* 草稿损坏就当没有 */
    }
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

function openEditor(date?: string) {
  if (date) day.value = new Date(`${date}T00:00:00`).getTime();
  editorOpen.value = true;
  void loadDay();
}

function closeEditor() {
  window.clearTimeout(draftTimer);
  flushDraft();
  editorOpen.value = false;
  void loadList();
}

defineExpose({ openEditor });

/* ---------- 实时草稿：输入即缓存到浏览器本地，防意外退出丢全文 ---------- */
const draftRestore = ref<{ content: string; images: string[] | null; at: number } | null>(null);

function draftKey(date: string) {
  return `note.draft.${date}`;
}

function flushDraft(date = dateStr()) {
  try {
    localStorage.setItem(
      draftKey(date),
      JSON.stringify({ content: content.value, images: images.value, at: Date.now() })
    );
  } catch {
    /* 存不下就算了 */
  }
}

function fmtAt(ts: number) {
  const d = new Date(ts);
  const hh = `${d.getHours()}`.padStart(2, "0");
  const mm = `${d.getMinutes()}`.padStart(2, "0");
  return `${d.getMonth() + 1}/${d.getDate()} ${hh}:${mm}`;
}

function restoreDraft() {
  const d = draftRestore.value;
  if (!d) return;
  content.value = d.content;
  if (d.images) {
    images.value = d.images;
    for (const name of d.images) void ensureThumb(dateStr(), name);
  }
  flushDraft();
  draftRestore.value = null;
  msg.value = "已恢复草稿（记得保存）";
}

function discardDraft() {
  try {
    localStorage.removeItem(draftKey(dateStr()));
  } catch {
    /* 忽略 */
  }
  draftRestore.value = null;
  msg.value = "草稿已丢弃";
}

let draftTimer = 0;
watch(content, () => {
  window.clearTimeout(draftTimer);
  draftTimer = window.setTimeout(() => flushDraft(), 500);
});
watch(images, () => flushDraft(), { deep: true });
// 切日期前把正在编辑的内容存到"旧日期"的草稿里
watch(day, (_nv, ov) => {
  window.clearTimeout(draftTimer);
  flushDraft(ymd(ov ?? Date.now()));
});

async function saveDay() {
  busy.value = true;
  err.value = "";
  msg.value = "";
  try {
    await invoke("notes_save", { date: dateStr(), content: content.value, images: images.value });
    msg.value = `已保存 ${dateStr()} 的日志`;
    try {
      localStorage.removeItem(draftKey(dateStr()));
    } catch {
      /* 忽略 */
    }
    draftRestore.value = null;
    coversStarted = false;
    await loadList();
    if (listMode.value === "gallery") void loadCovers();
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  } finally {
    busy.value = false;
  }
}

/* ---------- 工具栏：行转换 + 内联包裹，都对"选中文字/光标所在行"生效（Notion 式） ---------- */
const editorWrap = ref<HTMLElement | null>(null);

function ta(): HTMLTextAreaElement | null {
  return editorWrap.value?.querySelector("textarea") ?? null;
}

function applySel(fn: (t: string, s: number, e: number) => SelEdit) {
  const el = ta();
  const s = el?.selectionStart ?? content.value.length;
  const e = el?.selectionEnd ?? content.value.length;
  const r = fn(content.value, s, e);
  content.value = r.text;
  void nextTick(() => {
    const t = ta();
    if (t) {
      t.focus();
      t.setSelectionRange(r.selStart, r.selEnd);
    }
  });
}

const line = (kind: "heading" | "todo" | "list" | "quote" | "plain") =>
  applySel((t, s, e) => toggleLinePrefix(t, s, e, kind));
const wrap = (openStr: string, close: string) =>
  applySel((t, s, e) => wrapSelection(t, s, e, openStr, close));
const hr = () => applySel((t, s, e) => insertAtCursor(t, s, e, "---"));

/** 编辑器内 Ctrl+B / Ctrl+I（与 Notion 一致） */
function onEditorKey(e: KeyboardEvent) {
  if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey) return;
  const k = e.key.toLowerCase();
  if (k === "b") {
    e.preventDefault();
    wrap("**", "**");
  } else if (k === "i") {
    e.preventDefault();
    wrap("*", "*");
  }
}

/** 在末尾追加一块（图片这类整体块仍走追加，光标处由预览里 ↑↓ 调整顺序） */
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

// 从月历点一天过来 / 详细事项跳来：直接打开那天的编辑器
watch(
  () => props.dayTs,
  async (v) => {
    if (!v) return;
    day.value = v;
    editorOpen.value = true;
    await loadDay();
  }
);

onMounted(async () => {
  await loadList();
  void loadDay();
  // 默认就是画廊时 watch 不会触发；等列表取回后再加载封面（否则空列表直接跳过）
  if (listMode.value === "gallery") void loadCovers();
});
</script>

<template>
  <div class="notes">
    <!-- ===== 记录列表（默认主体）：列表 / 画廊 ===== -->
    <template v-if="!editorOpen">
      <div class="ov-bar">
        <div class="viewseg">
          <button :class="{ on: listMode === 'list' }" @click="listMode = 'list'">列表</button>
          <button :class="{ on: listMode === 'gallery' }" @click="listMode = 'gallery'">画廊</button>
        </div>
        <input v-model="searchText" class="ov-search" placeholder="搜索日期或正文…" />
        <span class="ov-cnt">{{ filteredNotes.length }} 篇</span>
        <button class="newbtn" @click="openEditor(today)">写今天的</button>
      </div>

      <!-- 列表 -->
      <div v-if="listMode === 'list'" class="ovlist">
        <div v-for="n in filteredNotes" :key="n.date" class="ovrow" @click="openEditor(n.date)">
          <span class="ovdate" :class="{ td: n.date === today }">{{ n.date }}</span>
          <span class="ovbrief">{{ brief(n.content).slice(0, 90) || "（无正文）" }}</span>
          <span v-if="n.images.length" class="ovimgs">{{ n.images.length }}图</span>
          <button class="mini edit" title="编辑这天" @click.stop="openEditor(n.date)">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M4 20h4L19 9l-4-4L4 16v4zM13 6l4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            <span>编辑</span>
          </button>
        </div>
        <span v-if="!filteredNotes.length" class="empty">
          {{ allNotes.length ? "没有匹配的日志" : "还没有日志，点右上角「写今天的」开始" }}
        </span>
      </div>

      <!-- 画廊（Notion 式：封面图 + 日期/标题） -->
      <div v-else class="ggrid">
        <button v-for="n in filteredNotes" :key="'g' + n.date" class="gcard" @click="openEditor(n.date)">
          <span class="gcover">
            <img v-if="covers[n.date]" :src="covers[n.date]" alt="" draggable="false" />
            <span v-else class="gnoimg">{{ brief(n.content).slice(0, 36) || "（空日志）" }}</span>
          </span>
          <span class="gmeta">
            <span class="gdate" :class="{ td: n.date === today }">{{ n.date }}</span>
            <span v-if="n.images.length" class="gcnt">{{ n.images.length }}图</span>
          </span>
          <span class="gtitle">{{ firstLine(n.content) || "（无标题）" }}</span>
        </button>
        <span v-if="!filteredNotes.length" class="empty">没有匹配的日志</span>
      </div>
    </template>

    <!-- ===== 编辑器（次级视图） ===== -->
    <template v-else>
      <div class="bar">
        <button class="mini back" @click="closeEditor">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M14.5 5.5L8 12l6.5 6.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>返回列表</span>
        </button>
        <NDatePicker
          v-model:value="day"
          type="date"
          :actions="['confirm']"
          :clearable="false"
          style="width: 150px"
          @update:value="loadDay"
        />
        <span class="d">{{ dateStr() }}{{ dateStr() === today ? " · 今天" : "" }}</span>
        <div class="acts">
          <NButton size="tiny" quaternary @click="importMd">导入 MD</NButton>
          <NButton size="tiny" quaternary @click="exportMd">导出全部 MD</NButton>
          <NButton size="small" type="primary" secondary :loading="busy" @click="saveDay">保存</NButton>
        </div>
      </div>

      <!-- 工具栏固定两行：文字效果 / 新图片宽度+视图切换 -->
      <div class="blocks">
        <button class="tb" title="选中行转标题（再点升级，### 后回到正文）" @click="line('heading')">标题</button>
        <button class="tb" title="转回普通段落" @click="line('plain')">正文</button>
        <button class="tb" title="选中行加待办框 / 取消" @click="line('todo')">待办</button>
        <button class="tb" title="选中行加项目符号 / 取消" @click="line('list')">列表</button>
        <button class="tb" title="选中行变引用 / 取消" @click="line('quote')">引用</button>
        <span class="tb-sep"></span>
        <button class="tb" title="选中文字加粗（Ctrl+B）" @click="wrap('**', '**')"><b>B</b></button>
        <button class="tb" title="选中文字斜体（Ctrl+I）" @click="wrap('*', '*')"><i>I</i></button>
        <button class="tb" title="选中文字下划线" @click="wrap('<u>', '</u>')"><u>U</u></button>
        <button class="tb" title="选中文字删除线" @click="wrap('~~', '~~')"><s>S</s></button>
        <button class="tb" title="行内代码" @click="wrap(String.fromCharCode(96), String.fromCharCode(96))">code</button>
        <button class="tb" title="链接" @click="wrap('[', '](链接地址)')">链接</button>
        <span class="tb-sep"></span>
        <button class="tb" title="在光标处插入分割线" @click="hr">分割线</button>
        <button class="tb" @click="addImage">图片…</button>
      </div>
      <div class="blocks subrow">
        <span class="tb-w">
          新图片宽度
          <button class="tb sm" :class="{ active: imgWidth === '33%' }" @click="imgWidth = '33%'">小</button>
          <button class="tb sm" :class="{ active: imgWidth === '60%' }" @click="imgWidth = '60%'">中</button>
          <button class="tb sm" :class="{ active: imgWidth === '100%' }" @click="imgWidth = '100%'">全宽</button>
        </span>
        <div class="viewseg">
          <button :class="{ on: noteView === 'edit' }" @click="noteView = 'edit'">编辑</button>
          <button :class="{ on: noteView === 'split' }" @click="noteView = 'split'">分栏</button>
          <button :class="{ on: noteView === 'preview' }" @click="noteView = 'preview'">预览</button>
        </div>
      </div>

      <!-- 未保存草稿提示条：意外退出也不丢内容 -->
      <div v-if="draftRestore" class="draftbar">
        <span>有 {{ fmtAt(draftRestore.at) }} 的未保存草稿</span>
        <button class="tb sm" @click="restoreDraft">恢复草稿</button>
        <button class="tb sm" @click="discardDraft">丢弃</button>
      </div>

      <div v-show="noteView !== 'preview'" ref="editorWrap" class="editor" @keydown="onEditorKey">
        <NInput
          v-model:value="content"
          type="textarea"
          placeholder="写点什么…也可以直接手写 Markdown；选中文字后点上面的按钮直接加格式"
          :autosize="{ minRows: 5, maxRows: 14 }"
        />
      </div>

      <div v-show="noteView !== 'edit'" class="preview">
        <MarkdownPreview
          :content="content"
          :images="imageMap"
          @update:content="(v: string) => (content = v)"
        />
      </div>

      <div v-if="images.length && noteView === 'edit'" class="imgs">
        <div v-for="name in images" :key="name" class="img">
          <img v-if="thumbs[`${dateStr()}/${name}`]" :src="thumbs[`${dateStr()}/${name}`]" alt="" />
          <span v-else class="none">读取中…</span>
          <button class="rm" title="从此天移除" @click="removeImage(name)">×</button>
        </div>
      </div>

      <p v-if="msg" class="ok">{{ msg }}</p>
      <p v-if="err" class="err">{{ err }}</p>
    </template>
  </div>
</template>

<style scoped>
.notes {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

/* ===== 记录列表 ===== */
.ov-bar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ov-search {
  flex: 1;
  min-width: 0;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  border-radius: var(--r-sm);
  padding: 5px 10px;
  font-size: 12px;
  font-family: inherit;
  outline: none;
}

.ov-search:focus {
  border-color: var(--accent-border);
}

.ov-cnt {
  flex: none;
  font-size: 11px;
  color: var(--text-faint);
}

.newbtn {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border: 1px solid var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent-text);
  border-radius: var(--r-full);
  font-size: 12px;
  font-family: inherit;
  padding: 4px 12px;
  cursor: pointer;
  transition: background var(--dur), color var(--dur);
}

.newbtn:hover {
  background: var(--accent-text);
  color: var(--surface);
}

.ovlist {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 420px;
  overflow-y: auto;
  padding-right: 4px;
}

.ovrow {
  display: flex;
  align-items: center;
  gap: 10px;
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-sm);
  padding: 7px 10px;
  cursor: pointer;
  transition: background var(--dur);
}

.ovrow:hover {
  background: var(--surface-hover);
}

.ovdate {
  flex: none;
  font-size: 11.5px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.ovdate.td {
  color: var(--accent-text);
  font-weight: 600;
}

.ovbrief {
  flex: 1;
  min-width: 0;
  font-size: 12px;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ovimgs {
  flex: none;
  font-size: 10px;
  color: var(--text-faint);
}

/* 行内编辑胶囊：默认圆形图标，悬停展开文字（与定位按钮同族） */
.mini.edit {
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 24px;
  padding: 0;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-full);
  cursor: pointer;
  overflow: hidden;
  transition: width var(--dur) cubic-bezier(0.22, 0.61, 0.36, 1), background var(--dur), color var(--dur), border-color var(--dur);
}

.mini.edit svg {
  width: 12px;
  height: 12px;
  flex: none;
}

.mini.edit span {
  max-width: 0;
  overflow: hidden;
  font-size: 11px;
  white-space: nowrap;
  opacity: 0;
  transition: max-width var(--dur) ease, opacity var(--dur) ease, margin-left var(--dur) ease;
}

.mini.edit:hover {
  width: 66px;
  color: var(--accent-text);
  border-color: var(--accent-border);
  background: var(--accent-soft);
}

.mini.edit:hover span {
  max-width: 34px;
  opacity: 1;
  margin-left: 4px;
}

/* ===== 画廊 ===== */
.ggrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
  gap: 10px;
  max-height: 420px;
  overflow-y: auto;
  padding: 2px 4px 2px 2px;
}

.gcard {
  display: flex;
  flex-direction: column;
  gap: 6px;
  text-align: left;
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-md);
  overflow: hidden;
  cursor: pointer;
  font-family: inherit;
  padding: 0 10px 8px;
  transition: border-color var(--dur), transform var(--dur);
}

.gcard:hover {
  border-color: var(--accent-border);
  transform: translateY(calc(-1px * var(--motion)));
}

.gcover {
  display: block;
  margin: 0 -10px;
  height: 96px;
  background: var(--surface-hover);
}

.gcover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.gnoimg {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  padding: 8px 12px;
  font-size: 11.5px;
  color: var(--text-faint);
  overflow: hidden;
}

.gmeta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 2px;
}

.gdate {
  font-size: 11px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.gdate.td {
  color: var(--accent-text);
  font-weight: 600;
}

.gcnt {
  font-size: 10px;
  color: var(--text-faint);
}

.gtitle {
  font-size: 12px;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ===== 编辑器 ===== */
.blocks {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}

.draftbar {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--warn);
  background: var(--warn-soft);
  border: 1px solid var(--warn);
  border-radius: var(--r-sm);
  padding: 6px 10px;
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

.blocks.subrow {
  justify-content: space-between;
}

.viewseg {
  display: inline-flex;
  gap: 2px;
  border: 1px solid var(--border);
  border-radius: var(--r-full);
  padding: 2px;
  background: var(--surface);
}

.viewseg button {
  border: 0;
  background: transparent;
  color: var(--text-faint);
  font-size: 11px;
  font-family: inherit;
  padding: 2px 12px;
  border-radius: var(--r-full);
  cursor: pointer;
  transition: background var(--dur), color var(--dur);
}

.viewseg button.on {
  color: var(--accent-text);
  background: var(--accent-soft);
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

/* 返回列表胶囊：与编辑/定位按钮同族 */
.mini.back {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-full);
  font-size: 11px;
  font-family: inherit;
  padding: 4px 10px;
  cursor: pointer;
  transition: color var(--dur), border-color var(--dur), background var(--dur);
}

.mini.back svg {
  width: 11px;
  height: 11px;
}

.mini.back:hover {
  color: var(--accent-text);
  border-color: var(--accent-border);
  background: var(--accent-soft);
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
  padding: 10px 0;
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
