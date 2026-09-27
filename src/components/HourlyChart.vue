<script setup lang="ts">
import * as echarts from "echarts";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import type { HourSlice } from "../types";
import { accentColor, chartColors, hexAlpha } from "../lib/chartColors";
import { isLight } from "../lib/uiState";

const props = withDefaults(
  defineProps<{ slices: HourSlice[]; height?: number; stacked?: boolean }>(),
  { height: 230, stacked: true }
);

const el = ref<HTMLDivElement | null>(null);
let chart: echarts.ECharts | null = null;
let ro: ResizeObserver | null = null;

/** 容器尺寸同步：页面用 v-show 隐藏时 clientWidth 为 0，
 *  此时初始化 ECharts 会得到一张 100px 宽的图（表现为"图表只有左半边、标签挤在一起"），
 *  所以宽度不够时先不初始化，交给 ResizeObserver 在真正可见时再来。 */
function syncSize() {
  if (!el.value) return;
  if (el.value.clientWidth < 8) return;
  if (!chart) render();
  else chart.resize();
}

/** 全天累计前 7 名，其余合并「其他」 */
const seriesData = computed(() => {
  const totals = new Map<string, number>();
  for (const s of props.slices) totals.set(s.appName, (totals.get(s.appName) ?? 0) + s.seconds);
  const ranked = [...totals.entries()].sort((a, b) => b[1] - a[1]);
  const top = new Set(ranked.slice(0, 7).map(([k]) => k));

  const seriesMap = new Map<string, number[]>();
  for (const s of props.slices) {
    const key = top.has(s.appName) ? s.appName : "其他";
    const arr = seriesMap.get(key) ?? new Array(24).fill(0);
    arr[s.hour] += Math.round(s.seconds / 60);
    seriesMap.set(key, arr);
  }
  const accent = accentColor();
  const ladder = [1, 0.72, 0.54, 0.4, 0.3, 0.22, 0.16];
  let rank = 0;
  const out = [...seriesMap.entries()]
    .sort((a, b) => (totals.get(b[0]) ?? Infinity) - (totals.get(a[0]) ?? Infinity))
    .map(([key, data]) => {
      const color =
        key === "其他" ? (isLight.value ? "#9aa2b4" : "#4b5563") : hexAlpha(accent, ladder[Math.min(rank, ladder.length - 1)]);
      rank++;
      return { name: key.replace(/\.exe$/i, ""), color, data };
    });
  return out;
});

function render() {
  if (!el.value || el.value.clientWidth < 8) return;
  if (!chart) chart = echarts.init(el.value);
  const c = chartColors();
  const series = seriesData.value;

  chart.setOption(
    {
      backgroundColor: "transparent",
      animation: false,
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "shadow" },
        backgroundColor: c.tipBg,
        borderColor: c.tipBorder,
        textStyle: { color: c.tipText, fontSize: 12 },
        valueFormatter: (v: number) => `${v} 分钟`,
      },
      // 图例改用自绘 HTML（见模板）：ECharts 内置图例在窄卡片里会压到绘图区上
      legend: { show: false },
      grid: { left: 4, right: 8, top: 10, bottom: 0, containLabel: true },
      xAxis: {
        type: "category",
        data: Array.from({ length: 24 }, (_, i) => `${i}`),
        axisLine: { lineStyle: { color: c.axis } },
        axisTick: { show: false },
        axisLabel: { color: c.label, fontSize: 10, interval: 2 },
      },
      yAxis: {
        type: "value",
        splitLine: { lineStyle: { color: c.split } },
        axisLabel: { color: c.label, fontSize: 10, formatter: "{value}分" },
      },
      series: series.map((s) => ({
        name: s.name,
        type: "bar" as const,
        stack: props.stacked ? "day" : undefined,
        barWidth: "62%",
        itemStyle: { color: s.color },
        data: s.data,
      })),
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
  if (el.value) {
    ro = new ResizeObserver(() => syncSize());
    ro.observe(el.value);
  }
});
onUnmounted(() => {
  window.removeEventListener("resize", onResize);
  ro?.disconnect();
  chart?.dispose();
  chart = null;
});
watch(() => [props.slices, isLight.value], render, { deep: true });
</script>

<template>
  <div class="hourly">
    <div v-if="seriesData.length" class="chart-legend">
      <span v-for="s in seriesData" :key="s.name" class="lg">
        <i :style="{ background: s.color }"></i>{{ s.name }}
      </span>
    </div>
    <div ref="el" class="chart" :style="{ height: height + 'px' }"></div>
  </div>
</template>

<style scoped>
.hourly {
  display: flex;
  flex-direction: column;
}

.chart {
  width: 100%;
}
</style>
