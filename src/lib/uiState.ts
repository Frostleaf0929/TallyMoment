import { ref } from "vue";

/**
 * 跨组件的界面状态（App.vue 写入，图表/排行等读取）
 * 图表用的是 ECharts 自绘 canvas，拿不到 CSS 变量，只能靠这些信号在主题切换时重绘
 */
export const isLight = ref(false);

/** 应用排行显示条数（设置页可调 5~20） */
export const appsTopN = ref(10);

/** 跨页跳转意图（今日卡片、应用排行 → 历史页/详细页） */
export const navIntent = ref<{ view: "recent" | "app"; app?: string; at: number } | null>(null);

export function jumpTo(view: "recent" | "app", app?: string) {
  navIntent.value = { view, app, at: Date.now() };
}
