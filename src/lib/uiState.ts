import { ref } from "vue";

/**
 * 跨组件的界面状态（App.vue 写入，图表/排行等读取）
 * 图表用的是 ECharts 自绘 canvas，拿不到 CSS 变量，只能靠这些信号在主题切换时重绘
 */
export const isLight = ref(false);

/** 应用排行显示条数（设置页可调 5~20） */
export const appsTopN = ref(10);

/** 跨页跳转意图（今日卡片、应用排行、某天弹层、任务名 → 历史页/详细页） */
export const navIntent = ref<{
  view: "recent" | "app" | "items" | "task";
  app?: string;
  date?: string;
  /** view = task 时：固定事项模板的任务 id */
  taskId?: number;
  at: number;
} | null>(null);

export function jumpTo(view: "recent" | "app" | "items" | "task", app?: string, date?: string, taskId?: number) {
  navIntent.value = { view, app, date, taskId, at: Date.now() };
}

/** 单纯切到某个页面（不传数据），用于"返回待办"这类按钮 */
export const tabIntent = ref<{ tab: string; at: number } | null>(null);

export function goTab(tab: string) {
  tabIntent.value = { tab, at: Date.now() };
}

/** 当前激活页（App.vue 同步写入；页面用它感知"自己被切到"，回来时刷新数据） */
export const activeTab = ref("today");

/** 要求待办页打开日志面板并定位到某天（详细·事项的"编辑"胶囊按钮用） */
export const notesIntent = ref<{ date: string; at: number } | null>(null);
