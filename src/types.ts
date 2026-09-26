export interface CurrentApp {
  displayName: string;
  seconds: number;
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
  apps: AppUsage[];
  hourly: HourSlice[];
  segments: SegSlice[];
}
