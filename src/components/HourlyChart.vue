<script setup lang="ts">
import * as echarts from "echarts";
import { onMounted, onUnmounted, ref, watch } from "vue";
import type { HourSlice } from "../types";
import { colorFor } from "../lib/colors";

const props = defineProps<{ slices: HourSlice[] }>();

const el = ref<HTMLDivElement | null>(null);
let chart: echarts.ECharts | null = null;

function render() {
  if (!el.value) return;
  if (!chart) chart = echarts.init(el.value);

  // 按全天累计取前 7 名应用，其余合并为「其他」
  const totals = new Map<string, number>();
  for (const s of props.slices) {
    totals.set(s.appName, (totals.get(s.appName) ?? 0) + s.seconds);
  }
  const ranked = [...totals.entries()].sort((a, b) => b[1] - a[1]);
  const top = new Set(ranked.slice(0, 7).map(([k]) => k));

  const seriesMap = new Map<string, number[]>();
  for (const s of props.slices) {
    const key = top.has(s.appName) ? s.appName : "其他";
    const arr = seriesMap.get(key) ?? new Array(24).fill(0);
    arr[s.hour] += Math.round(s.seconds / 60);
    seriesMap.set(key, arr);
  }
  const series = [...seriesMap.entries()]
    .sort((a, b) => {
      const ta = totals.get(a[0]) ?? Infinity;
      const tb = totals.get(b[0]) ?? Infinity;
      return tb - ta;
    })
    .map(([key, data]) => ({
      name: key.replace(/\.exe$/i, ""),
      type: "bar" as const,
      stack: "day",
      barWidth: "62%",
      itemStyle: {
        color: key === "其他" ? "#4b5563" : colorFor(key),
        borderRadius: key === seriesMap.keys().next().value ? [4, 4, 0, 0] : 0,
      },
      data,
    }));

  chart.setOption(
    {
      backgroundColor: "transparent",
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "shadow" },
        backgroundColor: "#161a24",
        borderColor: "#2a2f3d",
        textStyle: { color: "#e8eaf2", fontSize: 12 },
        valueFormatter: (v: number) => `${v} 分钟`,
      },
      legend: {
        top: 0,
        right: 0,
        icon: "circle",
        itemWidth: 8,
        itemHeight: 8,
        textStyle: { color: "#9aa3b8", fontSize: 11 },
        itemGap: 12,
      },
      grid: { left: 8, right: 8, top: 28, bottom: 0, containLabel: true },
      xAxis: {
        type: "category",
        data: Array.from({ length: 24 }, (_, i) => `${i}`),
        axisLine: { lineStyle: { color: "#2a2f3d" } },
        axisTick: { show: false },
        axisLabel: { color: "#5c6474", fontSize: 10, interval: 2 },
      },
      yAxis: {
        type: "value",
        splitLine: { lineStyle: { color: "#1c212d" } },
        axisLabel: { color: "#5c6474", fontSize: 10, formatter: "{value}分" },
      },
      series,
    },
    true
  );
}

function onResize() {
  chart?.resize();
}

onMounted(() => {
  render();
  window.addEventListener("resize", onResize);
});
onUnmounted(() => {
  window.removeEventListener("resize", onResize);
  chart?.dispose();
  chart = null;
});
watch(() => props.slices, render, { deep: true });
</script>

<template>
  <div ref="el" class="chart"></div>
</template>

<style scoped>
.chart {
  width: 100%;
  height: 230px;
}
</style>
