<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const stack = ["Tauri 2", "Vue 3", "TypeScript", "Pinia", "Naive UI", "ECharts"];

const seconds = ref(0);
const paused = ref(false);
const recording = ref(false);
const online = ref(false);
let timer: number | undefined;

async function refresh() {
  try {
    const s = await invoke<{ seconds: number; paused: boolean; recording: boolean }>(
      "today_summary"
    );
    seconds.value = s.seconds;
    paused.value = s.paused;
    recording.value = s.recording;
    online.value = true;
  } catch {
    online.value = false;
  }
}

onMounted(() => {
  refresh();
  timer = window.setInterval(refresh, 3000);
});
onUnmounted(() => clearInterval(timer));

const minutes = () => Math.floor(seconds.value / 60);
const statusText = () => (!online.value ? "未连接" : paused.value ? "已暂停" : "记录中");
</script>

<template>
  <main class="landing">
    <div class="badge">M1 · 核心时间记录</div>
    <h1>拾刻</h1>
    <p class="sub">TallyMoment</p>
    <p class="slogan">拾起每一刻，看清每一天</p>

    <div class="status">
      <div class="pulse" :class="{ off: paused || !online }"></div>
      <span>{{ statusText() }}</span>
      <span class="divider">·</span>
      <span class="num">{{ minutes() }}</span>
      <span>分钟今日</span>
    </div>

    <div class="stack">
      <span v-for="s in stack" :key="s">{{ s }}</span>
    </div>
    <p class="hint">关闭窗口会最小化到托盘，记录不会中断 · 托盘菜单可暂停记录</p>
  </main>
</template>

<style>
:root {
  font-family: "Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei", system-ui, sans-serif;
  font-size: 16px;
  color: #e8eaf2;
  background-color: #0f1117;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  user-select: none;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
}
</style>

<style scoped>
.landing {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  background:
    radial-gradient(600px 300px at 20% 0%, rgba(99, 102, 241, 0.16), transparent 60%),
    radial-gradient(600px 300px at 85% 100%, rgba(16, 185, 129, 0.12), transparent 60%),
    #0f1117;
}

.badge {
  font-size: 12px;
  letter-spacing: 0.08em;
  color: #a5b4fc;
  border: 1px solid rgba(99, 102, 241, 0.35);
  background: rgba(99, 102, 241, 0.1);
  border-radius: 999px;
  padding: 4px 14px;
}

h1 {
  margin: 18px 0 0;
  font-size: 56px;
  font-weight: 700;
  letter-spacing: 0.12em;
}

.sub {
  margin: 0;
  font-size: 15px;
  letter-spacing: 0.5em;
  color: #8b93a7;
  text-transform: uppercase;
}

.slogan {
  margin: 14px 0 0;
  color: #b6bdcc;
}

.status {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 24px;
  padding: 10px 20px;
  border: 1px solid #2a2f3d;
  background: rgba(22, 26, 36, 0.8);
  border-radius: 12px;
  color: #b6bdcc;
}

.pulse {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: #34d399;
  box-shadow: 0 0 0 0 rgba(52, 211, 153, 0.6);
  animation: pulse 2s infinite;
}

.pulse.off {
  background: #6b7280;
  animation: none;
  box-shadow: none;
}

@keyframes pulse {
  0% {
    box-shadow: 0 0 0 0 rgba(52, 211, 153, 0.5);
  }
  70% {
    box-shadow: 0 0 0 8px rgba(52, 211, 153, 0);
  }
  100% {
    box-shadow: 0 0 0 0 rgba(52, 211, 153, 0);
  }
}

.num {
  color: #34d399;
  font-weight: 700;
  font-size: 18px;
}

.stack {
  display: flex;
  gap: 8px;
  margin-top: 26px;
  flex-wrap: wrap;
  justify-content: center;
}

.stack span {
  font-size: 12px;
  color: #9aa3b8;
  border: 1px solid #2a2f3d;
  background: #161a24;
  border-radius: 6px;
  padding: 4px 10px;
}

.hint {
  margin-top: 30px;
  font-size: 12px;
  color: #5c6474;
}
</style>
