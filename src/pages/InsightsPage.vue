<script setup lang="ts">
import * as echarts from "echarts";
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { DailyTotal, Insight } from "../types";
import { fmtDuration } from "../lib/format";

const list = ref<Insight[]>([]);
const days = ref<DailyTotal[]>([]);
const loading = ref(true);

const el = ref<HTMLDivElement | null>(null);
let chart: echarts.ECharts | null = null;

async function load() {
  loading.value = true;
  try {
    list.value = await invoke<Insight[]>("insights");
    days.value = await invoke<DailyTotal[]>("recent_daily", { days: 7 });
    render();
  } catch {
    /* 忽略 */
  } finally {
    loading.value = false;
  }
}

function render() {
  if (!el.value) return;
  if (!chart) chart = echarts.init(el.value);
  chart.setOption(
    {
      backgroundColor: "transparent",
      tooltip: {
        trigger: "axis",
        backgroundColor: "#161a24",
        borderColor: "#2a2f3d",
        textStyle: { color: "#e8eaf2", fontSize: 12 },
        valueFormatter: (v: number) => fmtDuration(v),
      },
      grid: { left: 8, right: 8, top: 14, bottom: 0, containLabel: true },
      xAxis: {
        type: "category",
        data: days.value.map((d) => d.date.slice(5).replace("-", "/")),
        axisLine: { lineStyle: { color: "#2a2f3d" } },
        axisTick: { show: false },
        axisLabel: { color: "#5c6474", fontSize: 10 },
      },
      yAxis: {
        type: "value",
        splitLine: { lineStyle: { color: "#1c212d" } },
        axisLabel: {
          color: "#5c6474",
          fontSize: 10,
          formatter: (v: number) => `${Math.round(v / 60)}分`,
        },
      },
      series: [
        {
          type: "bar",
          barWidth: "46%",
          itemStyle: {
            borderRadius: [5, 5, 0, 0],
            color: {
              type: "linear",
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: "#34d399" },
                { offset: 1, color: "#10b98155" },
              ],
            },
          },
          data: days.value.map((d) => d.seconds),
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
  load();
  window.addEventListener("resize", onResize);
});
onUnmounted(() => {
  window.removeEventListener("resize", onResize);
  chart?.dispose();
  chart = null;
});
</script>

<template>
  <div class="insights">
    <header class="head">
      <h1>洞察</h1>
      <span class="sub">本地规则生成 · 不上传任何数据</span>
      <button class="refresh" :disabled="loading" @click="load">
        {{ loading ? "刷新中…" : "刷新" }}
      </button>
    </header>

    <section class="card wide">
      <h2>近 7 天使用时长</h2>
      <div ref="el" class="chart"></div>
    </section>

    <section class="list">
      <div v-for="(it, i) in list" :key="i" class="item" :class="it.tone">
        <span class="mark"></span>
        <div>
          <p class="t">{{ it.title }}</p>
          <p class="d">{{ it.detail }}</p>
        </div>
      </div>
      <p v-if="!list.length && !loading" class="empty">暂无洞察，先去「今日」看看有没有数据</p>
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
  color: #6b7280;
}

.refresh {
  margin-left: auto;
  border: 1px solid #2a2f3d;
  background: #161a24;
  color: #9aa3b8;
  border-radius: 8px;
  font-size: 12px;
  padding: 5px 12px;
  cursor: pointer;
  font-family: inherit;
}

.refresh:hover {
  color: #e8eaf2;
}

.card {
  border: 1px solid #232936;
  background: rgba(22, 26, 36, 0.72);
  border-radius: 14px;
  padding: 16px 18px;
}

.card h2 {
  margin: 0 0 8px;
  font-size: 13px;
  font-weight: 600;
  color: #9aa3b8;
}

.chart {
  width: 100%;
  height: 180px;
}

.list {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.item {
  display: flex;
  gap: 12px;
  border: 1px solid #232936;
  background: rgba(22, 26, 36, 0.72);
  border-radius: 12px;
  padding: 14px 16px;
}

.mark {
  flex: none;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-top: 6px;
  background: #6b7280;
}

.item.good .mark {
  background: #34d399;
}

.item.warn .mark {
  background: #fbbf24;
}

.item.info .mark {
  background: #818cf8;
}

.t {
  margin: 0 0 4px;
  font-size: 14px;
  font-weight: 600;
  color: #e8eaf2;
}

.d {
  margin: 0;
  font-size: 12px;
  color: #9aa3b8;
  line-height: 1.6;
}

.empty {
  color: #5c6474;
  font-size: 13px;
}
</style>
