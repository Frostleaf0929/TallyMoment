<script setup lang="ts">
import * as echarts from "echarts";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { InsightReport } from "../types";
import { fmtDuration } from "../lib/format";
import { chartColors } from "../lib/chartColors";
import { isLight } from "../lib/uiState";
import Icon from "../components/Icon.vue";
import DayBars from "../components/DayBars.vue";

const report = ref<InsightReport | null>(null);
const loading = ref(true);
const renderErr = ref("");

const chartEl = ref<HTMLDivElement | null>(null);
let chart: echarts.ECharts | null = null;

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
  const hourly14: { hour: number; appName: string; seconds: number }[] = [];
  for (let i = 13; i >= 0; i--) {
    const d = new Date(Date.now() - i * 86400000).toISOString().slice(0, 10);
    daily.push({ date: d, seconds: 3000 + Math.round(Math.sin(i) * 800) });
    inputDaily.push({ date: d, keys: 800 + i * 60, clicks: 300 + i * 20 });
    spans.push({ date: d, firstTs: now - i * 86400000 - 30000, lastTs: now - i * 86400000 + 40000 });
  }
  for (let h = 8; h < 22; h++) {
    hourly14.push({ hour: h, appName: "zcode.exe", seconds: 1800 });
  }
  return {
    blocks,
    daily,
    inputDaily,
    spans,
    hourly14,
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

/** 状态色：从主题令牌取，浅色主题下也是可读的深色（此前写死深色调色板） */
const stateColors = computed(() => {
  void isLight.value;
  const s = getComputedStyle(document.documentElement);
  const g = (k: string, d: string) => s.getPropertyValue(k).trim() || d;
  return {
    flow: g("--accent", "#7b84ec"),
    focused: g("--info", "#7d92cf"),
    fragmented: g("--warn", "#d3a95e"),
    keys: g("--accent", "#7b84ec"),
    clicks: g("--good", "#6fb59a"),
  };
});

function renderChart() {
  if (!chartEl.value || !report.value) return;
  if (!chart) chart = echarts.init(chartEl.value);
  const c = chartColors();
  const sc = stateColors.value;
  const days = report.value.inputDaily;
  const keySeries = days.map((d) => d.keys);
  const clickSeries = days.map((d) => d.clicks);
  chart.setOption(
    {
      backgroundColor: "transparent",
      tooltip: {
        trigger: "axis",
        backgroundColor: c.tipBg,
        borderColor: c.tipBorder,
        textStyle: { color: c.tipText, fontSize: 12 },
      },
      legend: { show: false },
      grid: { left: 4, right: 8, top: 10, bottom: 0, containLabel: true },
      xAxis: {
        type: "category",
        data: days.map((d) => d.date.slice(5).replace("-", "/")),
        axisLine: { lineStyle: { color: c.axis } },
        axisTick: { show: false },
        axisLabel: { color: c.label, fontSize: 10, interval: 1 },
      },
      yAxis: {
        type: "value",
        splitLine: { lineStyle: { color: c.split } },
        axisLabel: { color: c.label, fontSize: 10 },
      },
      series: [
        {
          name: "键入",
          type: "line",
          smooth: true,
          symbol: "circle",
          symbolSize: 5,
          data: keySeries,
          lineStyle: { color: sc.keys, width: 2 },
          itemStyle: { color: sc.keys },
          areaStyle: { color: sc.keys, opacity: 0.12 },
        },
        {
          name: "点击",
          type: "line",
          smooth: true,
          symbol: "circle",
          symbolSize: 5,
          data: clickSeries,
          lineStyle: { color: sc.clicks, width: 2 },
          itemStyle: { color: sc.clicks },
        },
      ],
    },
    true
  );
}

/** 自绘图例（ECharts 内置图例在窄卡片里会压到绘图区） */
const inputLegend = computed(() => {
  const sc = stateColors.value;
  return [
    { name: "键入", color: sc.keys },
    { name: "点击", color: sc.clicks },
  ];
});

const HOURS = Array.from({ length: 24 }, (_, i) => `${i}`);

/** 今日各小时按推断状态堆叠（粗粒度，替代原来的细条纹时间线） */
const stateBars = computed(() => {
  const flow = new Array(24).fill(0);
  const focused = new Array(24).fill(0);
  const fragmented = new Array(24).fill(0);
  for (const b of report.value?.blocks ?? []) {
    // 块内可能含空闲间隙：按活跃占比折算，避免虚高
    const ratio = b.span > 0 ? Math.min(1, b.seconds / b.span) : 1;
    let cur = b.startTs;
    while (cur < b.endTs) {
      const end = Math.min(Math.floor(cur / 3600) * 3600 + 3600, b.endTs);
      const minutes = ((end - cur) / 60) * ratio;
      const h = new Date(cur * 1000).getHours();
      if (b.state === "flow") flow[h] += minutes;
      else if (b.state === "focused") focused[h] += minutes;
      else fragmented[h] += minutes;
      cur = end;
    }
  }
  const r = (a: number[]) => a.map((v) => Math.round(v));
  const sc = stateColors.value;
  return {
    labels: HOURS,
    series: [
      { name: "心流", color: sc.flow, data: r(flow) },
      { name: "专注", color: sc.focused, data: r(focused) },
      { name: "碎片", color: sc.fragmented, data: r(fragmented) },
    ],
  };
});

/** 近 14 天作息分布：按小时累计（替代原来的每日活跃带细条） */
const hourBars = computed(() => {
  const arr = new Array(24).fill(0);
  for (const s of report.value?.hourly14 ?? []) arr[s.hour] += s.seconds / 60;
  return {
    labels: HOURS,
    series: [
      {
        name: "近 14 天累计",
        color: stateColors.value.flow,
        data: arr.map((v) => Math.round(v)),
      },
    ],
  };
});

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
// 主题切换时 canvas 需要重绘
watch(isLight, renderChart);

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

    <!-- 今日状态分布（按小时） -->
    <section class="glass-card wide">
      <h2>今日各小时状态（按推断状态堆叠）</h2>
      <DayBars :labels="stateBars.labels" :series="stateBars.series" />
      <p class="hint">
        块 = 间隔不足 5 分钟的使用归并；心流/专注/碎片为本地规则推断，不是精确值。
      </p>
    </section>

    <!-- 使用频率 -->
    <section class="glass-card wide">
      <h2>使用频率 · 近 14 天键入与点击</h2>
      <div class="chart-legend">
        <span v-for="l in inputLegend" :key="l.name" class="lg">
          <i :style="{ background: l.color }"></i>{{ l.name }}
        </span>
      </div>
      <div ref="chartEl" class="chart"></div>
    </section>

    <!-- 作息分布 -->
    <section class="glass-card wide">
      <h2>作息分布 · 近 14 天按小时累计</h2>
      <DayBars :labels="hourBars.labels" :series="hourBars.series" />
      <p class="hint">一眼看出你最常在哪些时段用电脑，适合用来安排需要专注的时段。</p>
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
