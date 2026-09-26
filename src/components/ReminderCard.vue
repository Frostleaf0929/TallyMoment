<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const title = ref("");
const message = ref("");
const visible = ref(false);
let unlisten: UnlistenFn | undefined;
let autoTimer: number | undefined;

async function dismiss() {
  try {
    await invoke("close_reminder");
  } catch {
    await getCurrentWebviewWindow().close();
  }
}

onMounted(async () => {
  unlisten = await listen<{ title: string; message: string }>(
    "reminder-show",
    (event) => {
      title.value = event.payload.title;
      message.value = event.payload.message;
      if (!visible.value) {
        visible.value = true;
        void getCurrentWebviewWindow().show();
      }
      if (autoTimer) clearTimeout(autoTimer);
      autoTimer = window.setTimeout(dismiss, 120_000);
    }
  );
});
onUnmounted(() => {
  unlisten?.();
  if (autoTimer) clearTimeout(autoTimer);
});
</script>

<template>
  <div class="reminder">
    <div class="head">
      <span class="mark">拾刻提醒</span>
      <button class="close" title="关闭" @click="dismiss">×</button>
    </div>
    <p class="title">{{ title }}</p>
    <p class="message">{{ message }}</p>
    <button class="ok" @click="dismiss">知道了</button>
  </div>
</template>

<style scoped>
.reminder {
  height: 100vh;
  display: flex;
  flex-direction: column;
  padding: 12px 14px;
  border: 1px solid #2c3345;
  border-radius: 12px;
  background:
    linear-gradient(160deg, rgba(99, 102, 241, 0.14), transparent 55%),
    #141824;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.45);
  user-select: none;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.mark {
  font-size: 11px;
  letter-spacing: 0.14em;
  color: #a5b4fc;
}

.close {
  border: 0;
  background: transparent;
  color: #6b7280;
  font-size: 16px;
  cursor: pointer;
  line-height: 1;
  padding: 2px 4px;
}

.close:hover {
  color: #e8eaf2;
}

.title {
  margin: 10px 0 2px;
  font-size: 16px;
  font-weight: 700;
  color: #e8eaf2;
}

.message {
  margin: 0;
  font-size: 12px;
  color: #9aa3b8;
}

.ok {
  margin-top: auto;
  align-self: flex-end;
  border: 1px solid rgba(99, 102, 241, 0.45);
  background: rgba(99, 102, 241, 0.18);
  color: #c7d2fe;
  font-size: 12px;
  border-radius: 8px;
  padding: 5px 14px;
  cursor: pointer;
}

.ok:hover {
  background: rgba(99, 102, 241, 0.3);
}
</style>
