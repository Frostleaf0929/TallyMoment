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
  flowSeconds: number;
  focusedSeconds: number;
  fragmentedSeconds: number;
  insights: Insight[];
}

export interface Task {
  id: number;
  content: string;
  priority: number;
  dueTs: number | null;
  done: boolean;
  doneTs: number | null;
  createdTs: number;
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
