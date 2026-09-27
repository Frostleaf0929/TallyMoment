<script setup lang="ts">
import * as echarts from "echarts";
import { onMounted, onUnmounted, ref, watch } from "vue";

/**
 * 通用块状柱图（24 小时分布风格）：给「分布 / 状态 / 趋势」共用
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
  }>(),
  { unit: "分钟", height: 230, stacked: true, labelInterval: 2 }
);

const el = ref<HTMLDivElement | null>(null);
let chart: echarts.ECharts | null = null;

function render() {
  if (!el.value) return;
  if (!chart) chart = echarts.init(el.value);

  const multi = props.series.length > 1;
  chart.setOption(
    {
      backgroundColor: "transparent",
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "shadow" },
        backgroundColor: "#161a24",
        borderColor: "#2a2f3d",
        textStyle: { color: "#e8eaf2", fontSize: 12 },
        valueFormatter: (v: number) => `${v} ${props.unit}`,
      },
      legend: {
        show: multi,
        top: 0,
        right: 0,
        icon: "circle",
        itemWidth: 8,
        itemHeight: 8,
        textStyle: { color: "#9aa3b8", fontSize: 11 },
        itemGap: 12,
      },
      grid: { left: 8, right: 8, top: multi ? 28 : 12, bottom: 0, containLabel: true },
      xAxis: {
        type: "category",
        data: props.labels,
        axisLine: { lineStyle: { color: "#2a2f3d" } },
        axisTick: { show: false },
        axisLabel: { color: "#5c6474", fontSize: 10, interval: props.labelInterval },
      },
      yAxis: {
        type: "value",
        splitLine: { lineStyle: { color: "#1c212d" } },
        axisLabel: {
          color: "#5c6474",
          fontSize: 10,
          formatter: (v: number) => `${v}${props.unit.slice(0, 1)}`,
        },
      },
      series: props.series.map((s) => ({
        name: s.name,
        type: "bar" as const,
        stack: props.stacked ? "total" : undefined,
        barWidth: "62%",
        itemStyle: { color: s.color, borderRadius: props.stacked ? 0 : 3 },
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
watch(() => [props.labels, props.series], render, { deep: true });
</script>

<template>
  <div ref="el" class="chart" :style="{ height: height + 'px' }"></div>
</template>

<style scoped>
.chart {
  width: 100%;
}
</style>
