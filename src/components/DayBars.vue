<script setup lang="ts">
import * as echarts from "echarts";
import { onMounted, onUnmounted, ref, watch } from "vue";
import { chartColors } from "../lib/chartColors";
import { isLight } from "../lib/uiState";

/**
 * 通用块状柱图（24 小时分布风格）：给「趋势 / 状态 / 分布」共用
 * 一柱一堆叠，避免细粒度热力条纹看不清
 */
export interface BarSeries {
  name: string;
  color: string;
  data: number[];
}

const props = withDefaults(
  defineProps<{
    labels: string[];
    series: BarSeries[];
    unit?: string;
    height?: number;
    stacked?: boolean;
    /** x 轴标签显示间隔 */
    labelInterval?: number;
    /** 单柱最大宽度：只有一两根柱子时不至于拉成一整块 */
    barMaxWidth?: number;
  }>(),
  { unit: "分钟", height: 230, stacked: true, labelInterval: 2, barMaxWidth: 46 }
);

const el = ref<HTMLDivElement | null>(null);
let chart: echarts.ECharts | null = null;

/** ECharts 画在 canvas 上，不认 CSS 变量 → 统一在这里解析成真实色值 */
function resolveColor(c: string): string {
  if (!c.startsWith("var(")) return c;
  const name = c.slice(4, -1).trim();
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "#7b84ec";
}

function render() {
  if (!el.value) return;
  if (!chart) chart = echarts.init(el.value);
  const c = chartColors();
  const multi = props.series.length > 1;

  chart.setOption(
    {
      backgroundColor: "transparent",
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "shadow" },
        backgroundColor: c.tipBg,
        borderColor: c.tipBorder,
        textStyle: { color: c.tipText, fontSize: 12 },
        valueFormatter: (v: number) => `${v} ${props.unit}`,
      },
      legend: { show: false },
      grid: { left: 4, right: 8, top: 10, bottom: 0, containLabel: true },
      xAxis: {
        type: "category",
        data: props.labels,
        axisLine: { lineStyle: { color: c.axis } },
        axisTick: { show: false },
        axisLabel: { color: c.label, fontSize: 10, interval: props.labelInterval },
      },
      yAxis: {
        type: "value",
        splitLine: { lineStyle: { color: c.split } },
        axisLabel: {
          color: c.label,
          fontSize: 10,
          formatter: (v: number) => `${v}${props.unit.slice(0, 1)}`,
        },
      },
      series: props.series.map((s) => ({
        name: s.name,
        type: "bar" as const,
        stack: props.stacked ? "total" : undefined,
        barMaxWidth: props.barMaxWidth,
        barWidth: multi ? "62%" : "46%",
        itemStyle: { color: resolveColor(s.color), borderRadius: props.stacked ? 0 : 4 },
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
});
onUnmounted(() => {
  window.removeEventListener("resize", onResize);
  chart?.dispose();
  chart = null;
});
watch(() => [props.labels, props.series, isLight.value], render, { deep: true });
</script>

<template>
  <div class="daybars">
    <div v-if="series.length > 1" class="chart-legend">
      <span v-for="s in series" :key="s.name" class="lg">
        <i :style="{ background: s.color }"></i>{{ s.name }}
      </span>
    </div>
    <div ref="el" class="chart" :style="{ height: height + 'px' }"></div>
  </div>
</template>

<style scoped>
.daybars {
  display: flex;
  flex-direction: column;
}

.chart {
  width: 100%;
}
</style>
