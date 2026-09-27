<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ReminderPayload } from "../types";

interface Toast extends ReminderPayload {
  remainingMs: number;
  deadline: number;
  paused: boolean;
}

const toasts = ref<Toast[]>([]);
let unlisten: UnlistenFn | undefined;
const timers = new Map<string, number>();

function scheduleAutoHide(t: Toast) {
  const old = timers.get(t.id);
  if (old) clearTimeout(old);
  if (t.sticky) return;
  const handle = window.setTimeout(() => remove(t.id), t.remainingMs);
  timers.set(t.id, handle);
}

function remove(id: string) {
  const i = toasts.value.findIndex((t) => t.id === id);
  if (i >= 0) toasts.value.splice(i, 1);
  const h = timers.get(id);
  if (h) clearTimeout(h);
  timers.delete(id);
  if (!toasts.value.length) void getCurrentWebviewWindow().hide();
}

function hover(id: string, entering: boolean) {
  const t = toasts.value.find((x) => x.id === id);
  if (!t || t.sticky) return;
  if (entering) {
    t.paused = true;
    const h = timers.get(id);
    if (h) clearTimeout(h);
    t.remainingMs = Math.max(0, t.deadline - Date.now());
  } else {
    t.paused = false;
    t.deadline = Date.now() + t.remainingMs;
    scheduleAutoHide(t);
  }
}

async function act(t: Toast, actionId: string) {
  try {
    await invoke("reminder_action", { kind: t.kind, refId: t.refId, action: actionId });
  } catch {
    /* 后端处理失败也不阻塞关卡 */
  }
  remove(t.id);
}

async function reportSize() {
  const h = toasts.value.length * 104 + (toasts.value.length ? 12 : 0);
  try {
    await invoke("reminder_resize", { height: Math.max(120, h) });
  } catch {
    /* 忽略 */
  }
}

function handlePayload(p: ReminderPayload) {
  const dup = toasts.value.find((t) => t.kind === p.kind && t.refId === p.refId);
  if (dup) {
    // 同源去重：原地刷新
    Object.assign(dup, p, {
      remainingMs: p.durationMs,
      deadline: Date.now() + p.durationMs,
      paused: false,
    });
  } else {
    toasts.value.unshift({
      ...p,
      remainingMs: p.durationMs,
      deadline: Date.now() + p.durationMs,
      paused: false,
    });
    if (toasts.value.length > 4) toasts.value.pop();
  }
  for (const t of toasts.value) scheduleAutoHide(t);
}

onMounted(async () => {
  // 先拉取窗口创建期间积压的提醒（确定性投递），再监听后续事件
  try {
    const pending = await invoke<ReminderPayload[]>("reminder_pending");
    pending.forEach(handlePayload);
  } catch {
    /* 忽略 */
  }
  if (toasts.value.length) void invoke("reminder_show_window");
  unlisten = await listen<ReminderPayload>("reminder-show", (event) => {
    handlePayload(event.payload);
    void invoke("reminder_show_window");
  });
  void reportSize();
});
onUnmounted(() => {
  unlisten?.();
  timers.forEach((h) => clearTimeout(h));
});
</script>

<template>
  <div class="stack">
    <div
      v-for="t in toasts"
      :key="t.id"
      class="toast"
      :style="{ '--ac': t.accent || 'var(--accent)' }"
      @mouseenter="hover(t.id, true)"
      @mouseleave="hover(t.id, false)"
    >
      <div class="bar"></div>
      <div class="body">
        <div class="head">
          <span class="mark">拾刻提醒</span>
          <button class="x" title="关闭" @click="remove(t.id)">×</button>
        </div>
        <p class="title">{{ t.title }}</p>
        <p class="msg">{{ t.body }}</p>
        <div class="foot">
          <div class="progress" :class="{ paused: t.paused }">
            <span
              class="fill"
              :style="{ 'animation-duration': t.durationMs + 'ms' }"
            ></span>
          </div>
          <div class="actions">
            <button
              v-for="a in t.actions"
              :key="a.id"
              class="act"
              :class="{ primary: a.id === 'done' || a.id === 'ack' }"
              @click="act(t, a.id)"
            >
              {{ a.label }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
.toast-root,
.toast-root body {
  margin: 0;
  background: transparent !important;
  overflow: hidden;
}
</style>

<style scoped>
.stack {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 8px 8px 0 8px;
}

.toast {
  display: flex;
  border-radius: 12px;
  overflow: hidden;
  background: rgba(22, 26, 34, 0.95);
  /* 去掉描边：桌面上会显出一圈灰框，改用外阴影托底 */
  border: 0;
  box-shadow: 0 14px 34px rgba(0, 0, 0, 0.46);
  user-select: none;
}

.bar {
  flex: none;
  width: 4px;
  background: var(--ac);
}

.body {
  flex: 1;
  min-width: 0;
  padding: 10px 12px 8px;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.mark {
  font-size: 10px;
  letter-spacing: 0.12em;
  color: var(--ac);
}

.x {
  border: 0;
  background: transparent;
  color: var(--text-faint);
  font-size: 15px;
  line-height: 1;
  cursor: pointer;
  padding: 0 3px;
}

.x:hover {
  color: var(--text);
}

.title {
  margin: 6px 0 2px;
  font-size: 14px;
  font-weight: 700;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.msg {
  margin: 0;
  font-size: 12px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.foot {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 8px;
}

.progress {
  flex: 1;
  height: 3px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.1);
  overflow: hidden;
}

.fill {
  display: block;
  height: 100%;
  background: var(--ac);
  animation: shrink linear forwards;
}

.progress.paused .fill {
  animation-play-state: paused;
}

@keyframes shrink {
  from {
    width: 100%;
  }
  to {
    width: 0;
  }
}

.actions {
  display: flex;
  gap: 6px;
}

.act {
  border: 1px solid var(--border-strong);
  background: transparent;
  color: var(--text-muted);
  font-size: 11px;
  font-family: inherit;
  border-radius: 8px;
  padding: 4px 10px;
  cursor: pointer;
}

.act:hover {
  color: var(--text);
  background: var(--surface-hover);
}

.act.primary {
  border-color: transparent;
  background: var(--ac);
  color: #fff;
}
</style>
