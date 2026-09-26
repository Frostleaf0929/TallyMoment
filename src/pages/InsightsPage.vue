<script setup lang="ts">
import * as echarts from "echarts";
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { InsightReport } from "../types";
import { fmtDuration } from "../lib/format";
import Icon from "../components/Icon.vue";

const report = ref<InsightReport | null>(null);
const loading = ref(true);
const renderErr = ref("");

const chartEl = ref<HTMLDivElement | null>(null);
let chart: echarts.ECharts | null = null;

const stateLabel: Record<string, string> = {
  flow: "心流",
  focused: "专注",
  fragmented: "碎片",
};

function fakeReport(): InsightReport {
  const now = Math.floor(Date.now() / 1000);
  const blocks = [
    { startTs: now - 14400, endTs: now - 12600, seconds: 1800, span: 1800, apps: ["zcode.exe", "msedge.exe"], switches: 2, state: "flow" as const },
    { startTs: now - 9000, endTs: now - 8400, seconds: 500, span: 600, apps: ["msedge.exe"], switches: 4, state: "focused" as const },
    { startTs: now - 4000, endTs: now - 3800, seconds: 150, span: 200, apps: ["explorer.exe", "zcode.exe"], switches: 5, state: "fragmented" as const },
  ];
  const daily: { date: string; seconds: number }[] = [];
  const inputDaily: { date: string; keys: number; clicks: number }[] = [];
  const spans: { date: string; firstTs: number; lastTs: number }[] = [];
  for (let i = 13; i >= 0; i--) {
    const d = new Date(Date.now() - i * 86400000).toISOString().slice(0, 10);
    daily.push({ date: d, seconds: 3000 + Math.round(Math.sin(i) * 800) });
    inputDaily.push({ date: d, keys: 800 + i * 60, clicks: 300 + i * 20 });
    spans.push({ date: d, firstTs: now - i * 86400000 - 30000, lastTs: now - i * 86400000 + 40000 });
  }
  return {
    blocks,
    daily,
    inputDaily,
    spans,
    flowSeconds: 1800,
    focusedSeconds: 500,
    fragmentedSeconds: 150,
    insights: [
      { title: "今天有 1 段心流，共 30 分钟", analysis: "最深的一段 10:02–10:32，主要在 ZCode。", suggestion: "深度工作优先安排到此时段。", tone: "good" },
      { title: "碎片化占 35%", analysis: "碎片 = 频繁切换、单块不足 10 分钟。", suggestion: "同类小事攒到固定时段批量处理。", tone: "warn" },
    ],
  };
}

async function load() {
  loading.value = true;
  try {
    if (localStorage.getItem("dev.fakeInsights") === "1") {
      report.value = fakeReport();
    } else {
      report.value = await invoke<InsightReport>("insights_report");
    }
    renderChart();
  } catch {
    /* 忽略 */
  } finally {
    loading.value = false;
  }
}

function renderChart() {
  if (!chartEl.value || !report.value) return;
  if (!chart) chart = echarts.init(chartEl.value);
  const days = report.value.inputDaily;
  chart.setOption(
    {
      backgroundColor: "transparent",
      tooltip: {
        trigger: "axis",
        backgroundColor: "#161a24",
        borderColor: "#2a2f3d",
        textStyle: { color: "#e8eaf2", fontSize: 12 },
      },
      legend: {
        top: 0,
        right: 0,
        icon: "rect",
        itemWidth: 10,
        itemHeight: 3,
        textStyle: { color: "#9aa3b8", fontSize: 11 },
      },
      grid: { left: 8, right: 8, top: 28, bottom: 0, containLabel: true },
      xAxis: {
        type: "category",
        data: days.map((d) => d.date.slice(5).replace("-", "/")),
        axisLine: { lineStyle: { color: "#2a2f3d" } },
        axisTick: { show: false },
        axisLabel: { color: "#5c6474", fontSize: 10 },
      },
      yAxis: {
        type: "value",
        splitLine: { lineStyle: { color: "#1c212d" } },
        axisLabel: { color: "#5c6474", fontSize: 10 },
      },
      series: [
        {
          name: "键入",
          type: "line",
          smooth: true,
          symbol: "circle",
          symbolSize: 5,
          data: days.map((d) => d.keys),
          lineStyle: { color: "#7b84ec", width: 2 },
          itemStyle: { color: "#7b84ec" },
          areaStyle: { color: "rgba(123,132,236,0.12)" },
        },
        {
          name: "点击",
          type: "line",
          smooth: true,
          symbol: "circle",
          symbolSize: 5,
          data: days.map((d) => d.clicks),
          lineStyle: { color: "#6fb59a", width: 2 },
          itemStyle: { color: "#6fb59a" },
        },
      ],
    },
    true
  );
}

function onResize() {
  chart?.resize();
}

onMounted(() => {
  window.addEventListener("error", (e) => (renderErr.value = e.message));
  window.addEventListener(
    "unhandledrejection",
    (e) => (renderErr.value = String(e.reason))
  );
  load();
  window.addEventListener("resize", onResize);
});
onUnmounted(() => {
  window.removeEventListener("resize", onResize);
  chart?.dispose();
  chart = null;
});

const dayStart = () => {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return Math.floor(d.getTime() / 1000);
};
const blocksWithPos = () =>
  (report.value?.blocks ?? []).map((b) => ({
    ...b,
    left: ((b.startTs - dayStart()) / 86400) * 100,
    width: Math.max(0.4, ((b.endTs - b.startTs) / 86400) * 100),
    tip: `${hhmm(b.startTs)}–${hhmm(b.endTs)} · ${b.apps
      .map((a) => a.replace(/\.exe$/i, ""))
      .join("、")} · ${fmtDuration(b.span)} · ${stateLabel[b.state] ?? b.state}`,
  }));
const nowPct = () => ((Date.now() / 1000 - dayStart()) / 86400) * 100;

function hhmm(ts: number): string {
  return new Date(ts * 1000).toTimeString().slice(0, 5);
}

const spanRows = () =>
  (report.value?.spans ?? []).map((s) => {
    const d0 = Math.floor(new Date(s.date + "T00:00:00").getTime() / 1000);
    return {
      date: s.date,
      label: s.date.slice(5).replace("-", "/"),
      left: ((s.firstTs - d0) / 86400) * 100,
      width: Math.max(0.5, ((s.lastTs - s.firstTs) / 86400) * 100),
      tip: `${hhmm(s.firstTs)} – ${hhmm(s.lastTs)}`,
    };
  });
</script>

<template>
  <div class="insights">
    <header class="head">
      <h1>洞察</h1>
      <span class="sub">本地规则推断 · 心流状态为估算，不上传任何数据</span>
      <button class="refresh" :disabled="loading" @click="load">
        {{ loading ? "分析中…" : "重新分析" }}
      </button>
    </header>

    <p v-if="renderErr" class="err">渲染错误: {{ renderErr }}</p>

    <!-- 状态摘要 -->
    <section class="cards">
      <div class="glass-card stat">
        <p class="label"><Icon name="fire" :size="14" /> 心流（推断）</p>
        <p class="value accent">{{ fmtDuration(report?.flowSeconds ?? 0) }}</p>
      </div>
      <div class="glass-card stat">
        <p class="label"><Icon name="target" :size="14" /> 专注（推断）</p>
        <p class="value">{{ fmtDuration(report?.focusedSeconds ?? 0) }}</p>
      </div>
      <div class="glass-card stat">
        <p class="label"><Icon name="doc" :size="14" /> 碎片（推断）</p>
        <p class="value">{{ fmtDuration(report?.fragmentedSeconds ?? 0) }}</p>
      </div>
    </section>

    <!-- 心流时间线 -->
    <section class="glass-card wide">
      <h2>今日专注块时间线（按推断状态着色）</h2>
      <div class="flow-strip">
        <div
          v-for="b in blocksWithPos()"
          :key="b.startTs"
          class="fblock"
          :class="b.state"
          :style="{ left: b.left + '%', width: b.width + '%' }"
          :title="b.tip"
        ></div>
        <div class="nowline" :style="{ left: nowPct() + '%' }"></div>
      </div>
      <div class="ruler">
        <span v-for="t in [0, 4, 8, 12, 16, 20, 24]" :key="t" class="tick" :style="{ left: (t / 24) * 100 + '%' }">
          {{ t }}时
        </span>
      </div>
      <div class="legend">
        <span><i class="ldot" style="background: var(--accent)"></i>心流</span>
        <span><i class="ldot" style="background: var(--info)"></i>专注</span>
        <span><i class="ldot" style="background: var(--warn)"></i>碎片</span>
        <span class="hint">块 = 间隔不足 5 分钟的使用归并；无色 = 未使用</span>
      </div>
    </section>

    <!-- 使用频率 -->
    <section class="glass-card wide">
      <h2>使用频率 · 近 14 天键入与点击</h2>
      <div ref="chartEl" class="chart"></div>
    </section>

    <!-- 作息带 -->
    <section class="glass-card wide">
      <h2>使用区间 · 每日活跃带（首末活动时刻）</h2>
      <div class="spans">
        <div v-for="s in spanRows()" :key="s.date" class="srow">
          <span class="sdate">{{ s.label }}</span>
          <div class="strack">
            <div class="sbar" :style="{ left: s.left + '%', width: s.width + '%' }" :title="s.tip"></div>
          </div>
        </div>
        <p v-if="!(report?.spans ?? []).length" class="empty">还没有足够数据</p>
      </div>
      <div class="ruler">
        <span v-for="t in [0, 4, 8, 12, 16, 20, 24]" :key="t" class="tick" :style="{ left: (t / 24) * 100 + '%' }">
          {{ t }}时
        </span>
      </div>
    </section>

    <!-- 分析与建议 -->
    <section class="analyses">
      <div v-for="(it, i) in report?.insights ?? []" :key="i" class="item" :class="it.tone">
        <span class="mark"></span>
        <div>
          <p class="t">{{ it.title }}</p>
          <p class="a">{{ it.analysis }}</p>
          <p class="s"><b>建议</b>{{ it.suggestion }}</p>
        </div>
      </div>
      <p v-if="!loading && !(report?.insights ?? []).length" class="empty">暂无分析，先积累一些数据</p>
    </section>
  </div>
</template>

<style scoped>
.insights {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.head {
  display: flex;
  align-items: center;
  gap: 12px;
}

.head h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
}

.sub {
  font-size: 12px;
  color: var(--text-muted);
}

.refresh {
  margin-left: auto;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: 8px;
  font-size: 12px;
  padding: 5px 12px;
  cursor: pointer;
  font-family: inherit;
}

.refresh:hover {
  color: var(--text);
}

.cards {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 14px;
}

.glass-card {
  padding: 16px 18px;
}

.glass-card.wide {
  padding-bottom: 12px;
}

.glass-card h2 {
  margin: 0 0 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

.stat .label {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 6px;
}

.stat .value {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.stat .value.accent {
  color: var(--accent-text);
}

.chart {
  width: 100%;
  height: 190px;
}

.flow-strip {
  position: relative;
  height: 34px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  overflow: hidden;
}

.fblock {
  position: absolute;
  top: 4px;
  bottom: 4px;
  border-radius: 4px;
  cursor: default;
}

.fblock.flow {
  background: var(--accent);
  opacity: 0.9;
}

.fblock.focused {
  background: var(--info);
  opacity: 0.6;
}

.fblock.fragmented {
  background: var(--warn);
  opacity: 0.45;
}

.fblock:hover {
  opacity: 1;
  outline: 1px solid var(--border-strong);
}

.nowline {
  position: absolute;
  top: 2px;
  bottom: 2px;
  width: 2px;
  background: #8a91a0;
  border-radius: 1px;
}

.ruler {
  position: relative;
  height: 16px;
  margin-top: 3px;
}

.tick {
  position: absolute;
  font-size: 10px;
  color: var(--text-faint);
  transform: translateX(-50%);
  white-space: nowrap;
}

.tick:first-child {
  transform: none;
}

.tick:last-child {
  transform: translateX(-100%);
}

.legend {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-top: 8px;
  font-size: 11px;
  color: var(--text-muted);
}

.legend .hint {
  margin-left: auto;
  color: var(--text-faint);
}

.ldot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 2px;
  margin-right: 4px;
  vertical-align: -1px;
}

.spans {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.srow {
  display: flex;
  align-items: center;
  gap: 10px;
}

.sdate {
  flex: none;
  width: 44px;
  font-size: 11px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.strack {
  flex: 1;
  position: relative;
  height: 12px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid var(--border);
  border-radius: var(--r-full);
}

.sbar {
  position: absolute;
  top: 2px;
  bottom: 2px;
  background: linear-gradient(90deg, var(--accent-soft), var(--accent));
  border-radius: var(--r-full);
}

.analyses {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.item {
  display: flex;
  gap: 12px;
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-md);
  padding: 14px 16px;
}

.mark {
  flex: none;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-top: 6px;
  background: var(--text-faint);
}

.item.good .mark {
  background: var(--good);
}

.item.warn .mark {
  background: var(--warn);
}

.item.info .mark {
  background: var(--info);
}

.t {
  margin: 0 0 4px;
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
}

.a {
  margin: 0 0 4px;
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.6;
}

.s {
  margin: 0;
  font-size: 12px;
  color: var(--text);
  line-height: 1.6;
}

.s b {
  color: var(--accent-text);
  margin-right: 6px;
  font-weight: 600;
}

.empty {
  color: var(--text-faint);
  font-size: 13px;
}
</style>
