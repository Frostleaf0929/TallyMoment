export interface CurrentApp {
  displayName: string;
  seconds: number;
  startTs: number;
  status: "focused" | "fragmented" | "rest" | "paused";
}

export interface AppUsage {
  name: string;
  displayName: string;
  seconds: number;
}

export interface HourSlice {
  hour: number;
  appName: string;
  seconds: number;
}

export interface SegSlice {
  appName: string;
  startTs: number;
  endTs: number;
  title: string;
}

export interface DayReport {
  date: string;
  totalSeconds: number;
  appCount: number;
  recording: boolean;
  paused: boolean;
  current: CurrentApp | null;
  blockIndex: number;
  keys: number;
  clicks: number;
  apps: AppUsage[];
  hourly: HourSlice[];
  segments: SegSlice[];
}

export interface DailyTotal {
  date: string;
  seconds: number;
}

/** 周期报表（历史页 按月/按年/总计） */
export interface PeriodBucket {
  label: string;
  date: string;
  seconds: number;
}

export interface PeriodReport {
  kind: "month" | "year" | "all";
  key: string;
  title: string;
  totalSeconds: number;
  appCount: number;
  activeDays: number;
  keys: number;
  clicks: number;
  apps: AppUsage[];
  hourly: HourSlice[];
  buckets: PeriodBucket[];
}

export interface PeriodIndex {
  months: string[];
  years: string[];
}

/** 单个应用的周期报表（历史页「按应用」） */
export interface AppPeriodReport {
  name: string;
  displayName: string;
  kind: "day" | "month" | "year" | "all";
  key: string;
  title: string;
  totalSeconds: number;
  activeDays: number;
  buckets: PeriodBucket[];
}

/** 任意区间报表（详细页） */
export interface RangeReport {
  from: string;
  to: string;
  title: string;
  days: number;
  totalSeconds: number;
  appCount: number;
  activeDays: number;
  avgPerDay: number;
  keys: number;
  clicks: number;
  apps: AppUsage[];
  hourly: HourSlice[];
  buckets: PeriodBucket[];
  app: string | null;
  appDisplay: string | null;
}

/** 界面偏好 */
export interface Prefs {
  appsTopN: number;
}

/** 数据目录与文件信息（设置页） */
export interface DataInfo {
  dir: string;
  dbPath: string;
  dbBytes: number;
  walBytes: number;
  fallback: boolean;
  exists: boolean;
}

export interface Insight {
  title: string;
  analysis: string;
  suggestion: string;
  tone: "good" | "warn" | "info";
}

export interface BlockView {
  startTs: number;
  endTs: number;
  seconds: number;
  span: number;
  apps: string[];
  switches: number;
  state: "flow" | "focused" | "fragmented";
}

export interface SpanDay {
  date: string;
  firstTs: number;
  lastTs: number;
}

export interface InsightReport {
  blocks: BlockView[];
  daily: { date: string; seconds: number }[];
  inputDaily: { date: string; keys: number; clicks: number }[];
  spans: SpanDay[];
  /** 近 14 天按小时的累计使用时长 */
  hourly14: HourSlice[];
  flowSeconds: number;
  focusedSeconds: number;
  fragmentedSeconds: number;
  insights: Insight[];
}

/** 习惯追踪：固定事项某天的完成格 */
export interface HabitCell {
  date: string;
  done: boolean;
  /** 完成（开始/创建 → 完成）用时分钟数 */
  spentMin: number | null;
}

/** 习惯追踪：一个固定事项的完成格行 */
export interface HabitRow {
  taskId: number;
  content: string;
  repeatMode: string;
  cells: HabitCell[];
}

export interface Task {
  id: number;
  content: string;
  priority: number;
  dueTs: number | null;
  done: boolean;
  doneTs: number | null;
  createdTs: number;
  /** 固定事项重复方式："" | daily | weekly | monthly | yearly */
  repeatMode: string;
  /** 由哪个固定事项模板生成（模板自身为 null） */
  templateId: number | null;
  /** 开始做的时间（每日追踪） */
  startTs: number | null;
  /** 该条本身是固定事项模板 */
  isTemplate: boolean;
}

export interface ReminderRule {
  id: number;
  title: string;
  body: string;
  mode: "interval" | "daily";
  intervalMinutes: number | null;
  dailyTimes: string[];
  sticky: boolean;
  cardDurationSec: number;
  accentColor: string | null;
  enabled: boolean;
  /** card | fullscreen */
  style: string;
}

export interface TodoStats {
  todayDone: number;
  weekDone: number;
  weekRate: number;
  ontimeRate: number;
  avgMinutes: number;
  buckets: number[];
}

export interface ReminderAction {
  id: string;
  label: string;
}

export interface ReminderPayload {
  id: string;
  kind: "rule" | "task";
  refId: number;
  title: string;
  body: string;
  sticky: boolean;
  durationMs: number;
  accent: string | null;
  actions: ReminderAction[];
}

export interface PetSettings {
  scale: number;
  opacity: number;
  alwaysOnTop: boolean;
  passThrough: boolean;
  mirror: boolean;
  posX: number;
  posY: number;
  activeModel: string;
  activeModelDir: string | null;
  mode: "keyboard" | "standard";
}
