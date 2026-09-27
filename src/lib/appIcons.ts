import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";

/** 应用图标缓存：name(进程标识) -> { url: 图标(dataURL), color: 主色 } */
export const iconCache = reactive<Record<string, { url: string; color: string }>>({});

const pending = new Set<string>();
let timer: number | undefined;

interface RawIcon {
  name: string;
  w: number;
  h: number;
  /** BGRA base64 */
  bgra: string;
  color: string;
}

/** BGRA -> PNG dataURL（浏览器侧转换，Rust 不引 PNG 编码器） */
function toDataUrl(bgra: string, w: number, h: number): string {
  const bin = atob(bgra);
  const px = new Uint8ClampedArray(bin.length);
  for (let i = 0; i < bin.length; i++) px[i] = bin.charCodeAt(i);
  const cv = document.createElement("canvas");
  cv.width = w;
  cv.height = h;
  const ctx = cv.getContext("2d");
  if (!ctx) return "";
  const img = ctx.createImageData(w, h);
  for (let i = 0, j = 0; i < px.length; i += 4, j += 4) {
    img.data[j] = px[i + 2];
    img.data[j + 1] = px[i + 1];
    img.data[j + 2] = px[i];
    img.data[j + 3] = px[i + 3];
  }
  ctx.putImageData(img, 0, 0);
  return cv.toDataURL("image/png");
}

/** 批量请求缺失的图标（60ms 合并一次请求） */
export function requestIcons(names: string[]) {
  const need = names.filter((n) => n && !iconCache[n] && !pending.has(n));
  if (!need.length) return;
  need.forEach((n) => pending.add(n));
  if (timer) window.clearTimeout(timer);
  timer = window.setTimeout(async () => {
    const batch = [...pending];
    pending.clear();
    try {
      const list = await invoke<RawIcon[]>("app_icons", { names: batch });
      const got = new Set<string>();
      for (const it of list) {
        iconCache[it.name] = { url: toDataUrl(it.bgra, it.w, it.h), color: it.color };
        got.add(it.name);
      }
      // 读不到的也记一笔，避免反复请求
      for (const n of batch) if (!got.has(n)) iconCache[n] = { url: "", color: "" };
    } catch {
      /* 忽略：图标只是装饰 */
    }
  }, 60);
}

export const iconUrl = (name: string) => iconCache[name]?.url || "";
export const iconColor = (name: string) => iconCache[name]?.color || "";
