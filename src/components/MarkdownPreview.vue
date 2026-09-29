<script setup lang="ts">
import { computed } from "vue";
import { moveBlock, parseBlocks, removeBlock, renderLine } from "../lib/mdBlocks";

/** 块式预览：按块渲染 Markdown，并支持块的上移/下移/删除（回写到正文） */
const props = defineProps<{
  content: string;
  /** 附件名 -> dataURL */
  images: Record<string, string>;
}>();
const emit = defineEmits<{ (e: "update:content", v: string): void }>();

const blocks = computed(() =>
  parseBlocks(props.content).map((b) => ({
    kind: b.kind,
    html: b.lines.map((ln) => renderLine(ln, props.images)).join("<br />"),
  }))
);

function up(i: number) {
  emit("update:content", moveBlock(props.content, i, -1));
}
function down(i: number) {
  emit("update:content", moveBlock(props.content, i, 1));
}
function del(i: number) {
  emit("update:content", removeBlock(props.content, i));
}
</script>

<template>
  <div class="mdprev">
    <p v-if="!blocks.length" class="empty">还没有内容，用上面的按钮插入块试试</p>
    <div v-for="(b, i) in blocks" :key="i" class="mdblock" :class="`k-${b.kind}`">
      <div class="mdbody" v-html="b.html"></div>
      <div class="mdacts">
        <button class="mb" title="上移" :disabled="i === 0" @click="up(i)">↑</button>
        <button class="mb" title="下移" :disabled="i === blocks.length - 1" @click="down(i)">↓</button>
        <button class="mb del" title="删除这一块" @click="del(i)">×</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mdprev {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.mdblock {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  border: 1px solid transparent;
  border-radius: var(--r-sm);
  padding: 4px 6px;
  transition: background var(--dur), border-color var(--dur);
}

.mdblock:hover {
  background: var(--surface);
  border-color: var(--border);
}

.mdbody {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  line-height: 1.75;
  color: var(--text);
  word-break: break-word;
}

.mdacts {
  flex: none;
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity var(--dur);
}

.mdblock:hover .mdacts {
  opacity: 1;
}

.mb {
  width: 20px;
  height: 20px;
  border: 0;
  border-radius: var(--r-sm);
  background: var(--surface-hover);
  color: var(--text-muted);
  font-size: 11px;
  line-height: 1;
  cursor: pointer;
  font-family: inherit;
}

.mb:hover {
  color: var(--text);
}

.mb:disabled {
  opacity: 0.35;
  cursor: default;
}

.mb.del:hover {
  color: var(--danger);
}

/* 各块的呈现 */
.k-heading .mdbody {
  font-size: 16px;
  font-weight: 700;
}

.k-quote .mdbody {
  border-left: 3px solid var(--accent-border);
  padding-left: 10px;
  color: var(--text-muted);
}

.k-hr .mdbody {
  border-top: 1px solid var(--border);
  height: 1px;
  padding: 0;
}

.k-todo .mdbody :deep(code),
.k-list .mdbody :deep(code) {
  background: var(--surface-hover);
  border-radius: 4px;
  padding: 1px 5px;
  font-size: 12px;
}

.mdbody :deep(.mdimg) {
  max-width: min(60%, 300px);
  max-height: 210px;
  border-radius: var(--r-sm);
  display: block;
  margin: 4px 0;
  object-fit: contain;
}

.mdbody :deep(.miss) {
  color: var(--text-faint);
  font-size: 12px;
}

.empty {
  margin: 0;
  font-size: 12px;
  color: var(--text-faint);
}
</style>
