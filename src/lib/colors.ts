const PALETTE = [
  "#6366f1",
  "#10b981",
  "#f59e0b",
  "#ec4899",
  "#06b6d4",
  "#8b5cf6",
  "#ef4444",
  "#84cc16",
  "#f97316",
  "#14b8a6",
];

const cache = new Map<string, string>();

/** 按应用名稳定分配调色板颜色 */
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
