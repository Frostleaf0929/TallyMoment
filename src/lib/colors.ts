// 柔和去饱和应用色板（v2：配合热力时间线与柔和图表）
const PALETTE = [
  "#7b84ec",
  "#6fb59a",
  "#c9a06a",
  "#c98ba0",
  "#6ba3c9",
  "#9d8fc9",
  "#c98a8a",
  "#8fb37e",
  "#c9b06a",
  "#7aa3b5",
];

const cache = new Map<string, string>();

/** 按应用名稳定分配柔和色 */
export function colorFor(key: string): string {
  const hit = cache.get(key);
  if (hit) return hit;
  let hash = 0;
  for (let i = 0; i < key.length; i++) {
    hash = (hash * 31 + key.charCodeAt(i)) | 0;
  }
  const color = PALETTE[Math.abs(hash) % PALETTE.length];
  cache.set(key, color);
  return color;
}
