<script setup lang="ts">
const stack = ["Tauri 2", "Vue 3", "TypeScript", "Pinia", "Naive UI", "ECharts"];
</script>

<template>
  <main class="landing">
    <div class="badge">M0 · 脚手架就绪</div>
    <h1>拾刻</h1>
    <p class="sub">TallyMoment</p>
    <p class="slogan">拾起每一刻，看清每一天</p>
    <div class="stack">
      <span v-for="s in stack" :key="s">{{ s }}</span>
    </div>
    <p class="hint">窗口与托盘运行中 · 点右上角 × 会最小化到托盘，托盘图标可再次打开</p>
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
