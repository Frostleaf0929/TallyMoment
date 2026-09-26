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
  detail: string;
  tone: "good" | "warn" | "info";
}
