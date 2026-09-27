/**
 * ECharts 用的是自绘 canvas，读不到 CSS 变量，必须显式取色。
 * 取的都是 theme.css 里定义的 --chart-* 令牌，深浅主题各一套，
 * 这样图表轴线/标签/提示框会跟着主题走（此前恒用深色，浅色模式下像黑线）。
 */
export interface ChartColors {
  axis: string;
  label: string;
  split: string;
  rail: string;
  tipBg: string;
  tipBorder: string;
  tipText: string;
}

export function chartColors(): ChartColors {
  const s = getComputedStyle(document.documentElement);
  const g = (key: string, fallback: string) => s.getPropertyValue(key).trim() || fallback;
  return {
    axis: g("--chart-axis", "#2a2f3d"),
    label: g("--chart-label", "#5c6474"),
    split: g("--chart-split", "#1c212d"),
    rail: g("--chart-rail", "rgba(255,255,255,0.09)"),
    tipBg: g("--chart-tip-bg", "#161a24"),
    tipBorder: g("--chart-tip-border", "#2a2f3d"),
    tipText: g("--chart-tip-text", "#e8eaf2"),
  };
}

/** 强调色（用于同色系阶梯） */
export function accentColor(): string {
  const s = getComputedStyle(document.documentElement);
  return s.getPropertyValue("--accent").trim() || "#7b84ec";
}

export function hexAlpha(hex: string, a: number): string {
  const h = hex.replace("#", "");
  const full = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  const r = parseInt(full.slice(0, 2), 16);
  const g = parseInt(full.slice(2, 4), 16);
  const b = parseInt(full.slice(4, 6), 16);
  return `rgba(${r},${g},${b},${a})`;
}
