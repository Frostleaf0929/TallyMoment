import { computed, ref } from "vue";

/**
 * 外观状态单一真源（App.vue 负责把 token 写到 <html>，个性化页负责改这里的值）
 * 参考 dsh-dream-skin 的两条规则：
 *  1) 所有"有多透"的控制收在一个组里，不散落在各页面
 *  2) 材质档位只改风格（模糊系数/填充），绝不动用户调过的滑杆数值
 */

export type ThemePref = "dark" | "light" | "system";
export type Material = "frosted" | "liquid";
export type WallKind = "none" | "gradient" | "image";

/** 外观参数版本：默认值变了要让旧值一次性失效 */
export const UI_VERSION = "4";
const stale = () => localStorage.getItem("ui.v") !== UI_VERSION;

const num = (key: string, fallback: number) => {
  const v = localStorage.getItem(key);
  const n = v === null ? NaN : Number(v);
  return Number.isFinite(n) ? n : fallback;
};

export const themePref = ref<ThemePref>((localStorage.getItem("ui.theme") as ThemePref) || "dark");
export const accent = ref(localStorage.getItem("ui.accent") || "indigo");
export const customAccent = ref(localStorage.getItem("ui.customAccent") || "#7b84ec");
export const glass = ref(localStorage.getItem("ui.glass") !== "off");
export const material = ref<Material>(
  (localStorage.getItem("ui.material") as Material) || "frosted"
);
export const blur = ref(num("ui.blur", 26));
/* 背景层默认完全不透明（用户选定：直角 + 不透明），滑杆仍可回调 */
export const bgAlpha = ref(stale() ? 100 : num("ui.bgAlpha", 100));
export const sideAlpha = ref(stale() ? 50 : num("ui.sideAlpha", 50));
export const cardAlpha = ref(num("ui.cardAlpha", 100));
/* 三块表面各自的模糊与阴影（个性化页分三组调） */
export const bgBlur = ref(stale() ? 18 : num("ui.bgBlur", 18));
export const sideBlur = ref(stale() ? 26 : num("ui.sideBlur", 26));
export const cardBlur = ref(stale() ? 26 : num("ui.cardBlur", 26));
export const sideShadow = ref(num("ui.sideShadow", 0));
export const cardShadow = ref(num("ui.cardShadow", 1));
/** 应用名模式：true = 全英文（exe 名），false = 跟随系统显示名 */
export const appNameEnglish = ref(localStorage.getItem("ui.appNameEn") === "1");
/** 应用配色模式：random 随机色 / accent 跟随强调色 / iconColor 按图标取色 / icon 直接显示图标 */
export type AppColorMode = "random" | "accent" | "iconColor" | "icon";
export const appColorMode = ref<AppColorMode>(
  (localStorage.getItem("ui.appColor") as AppColorMode) || "random"
);
export const motion = ref(num("ui.motion", 1));
export const wallKind = ref<WallKind>(
  (localStorage.getItem("ui.wallKind") as WallKind) || "gradient"
);
/** 壁纸图片（后端读成 base64 后前端转 Blob URL） */
export const wallImageUrl = ref<string | null>(null);
export const systemDark = ref(true);

export const isDark = () =>
  themePref.value === "dark" || (themePref.value === "system" && systemDark.value);

/** 皮肤预设：一次改主题+强调色+材质+透明度的组合（对标参考项目的「皮肤」行） */
export interface Skin {
  key: string;
  label: string;
  theme: ThemePref;
  accent: string;
  material: Material;
  blur: number;
  bgAlpha: number;
  sideAlpha: number;
  cardAlpha: number;
  /** 预览小样用色：底色 / 卡片 / 强调 */
  swatch: [string, string, string];
}

export const skins: Skin[] = [
  {
    key: "indigo",
    label: "默认靛蓝",
    theme: "dark",
    accent: "indigo",
    material: "frosted",
    blur: 26,
    bgAlpha: 80,
    sideAlpha: 50,
    cardAlpha: 100,
    swatch: ["#141821", "#1d2230", "#7b84ec"],
  },
  {
    key: "nebula",
    label: "星云紫",
    theme: "dark",
    accent: "violet",
    material: "liquid",
    blur: 34,
    bgAlpha: 72,
    sideAlpha: 40,
    cardAlpha: 92,
    swatch: ["#171327", "#241d3a", "#a08fe0"],
  },
  {
    key: "aurora",
    label: "极光青",
    theme: "dark",
    accent: "teal",
    material: "frosted",
    blur: 28,
    bgAlpha: 76,
    sideAlpha: 44,
    cardAlpha: 96,
    swatch: ["#101b1f", "#183036", "#58b3c4"],
  },
  {
    key: "ember",
    label: "余烬橙",
    theme: "dark",
    accent: "amber",
    material: "frosted",
    blur: 22,
    bgAlpha: 82,
    sideAlpha: 52,
    cardAlpha: 100,
    swatch: ["#1c1710", "#2a2118", "#d3a35e"],
  },
  {
    key: "midnight",
    label: "午夜黑",
    theme: "dark",
    accent: "indigo",
    material: "liquid",
    blur: 18,
    bgAlpha: 92,
    sideAlpha: 62,
    cardAlpha: 100,
    swatch: ["#0b0d12", "#12151c", "#8f97f0"],
  },
  {
    key: "paper",
    label: "苍白纸",
    theme: "light",
    accent: "indigo",
    material: "frosted",
    blur: 24,
    bgAlpha: 78,
    sideAlpha: 46,
    cardAlpha: 100,
    swatch: ["#f2f4f8", "#ffffff", "#6a74e0"],
  },
  {
    key: "bright",
    label: "干净明亮",
    theme: "light",
    accent: "teal",
    material: "frosted",
    blur: 20,
    bgAlpha: 84,
    sideAlpha: 52,
    cardAlpha: 100,
    swatch: ["#eef6f7", "#ffffff", "#2f8f9f"],
  },
  {
    key: "sakura",
    label: "樱花粉",
    theme: "light",
    accent: "rose",
    material: "liquid",
    blur: 32,
    bgAlpha: 70,
    sideAlpha: 40,
    cardAlpha: 90,
    swatch: ["#fbf1f4", "#ffffff", "#d3859b"],
  },
];

export function applySkin(s: Skin) {
  themePref.value = s.theme;
  accent.value = s.accent;
  material.value = s.material;
  blur.value = s.blur;
  bgAlpha.value = s.bgAlpha;
  sideAlpha.value = s.sideAlpha;
  cardAlpha.value = s.cardAlpha;
}

/** 当前是否与某套皮肤一致（用于打勾） */
export function activeSkinKey(): string | null {
  const hit = skins.find(
    (s) =>
      s.theme === themePref.value &&
      s.accent === accent.value &&
      s.material === material.value &&
      Math.abs(s.blur - blur.value) < 0.5
  );
  return hit ? hit.key : null;
}

const ACCENTS = ["indigo", "teal", "green", "violet", "amber", "rose"];

/** 把 token 写到 <html>；App.vue 在 watchEffect 里调用 */
export function applyAppearance() {
  const root = document.documentElement;
  root.dataset.theme = isDark() ? "dark" : "light";
  root.dataset.accent = ACCENTS.includes(accent.value) ? accent.value : "custom";
  root.dataset.material = material.value;

  if (!ACCENTS.includes(accent.value)) {
    // 自定义取色器：直接写三个派生 token
    root.style.setProperty("--accent", accent.value);
    root.style.setProperty("--accent-soft", hexAlpha(accent.value, 0.16));
    root.style.setProperty("--accent-text", isDark() ? lighten(accent.value, 0.25) : darken(accent.value, 0.2));
    root.style.setProperty("--accent-border", hexAlpha(accent.value, 0.38));
    root.style.setProperty("--wall-a", hexAlpha(accent.value, 0.2));
  } else {
    for (const k of ["--accent", "--accent-soft", "--accent-text", "--accent-border", "--wall-a"]) {
      root.style.removeProperty(k);
    }
  }

  root.style.setProperty("--glass-blur", `${blur.value}px`);
  root.style.setProperty("--wall-blur", `${Math.round(blur.value * 0.7)}px`);
  root.style.setProperty("--bg-blur", `${bgBlur.value}px`);
  root.style.setProperty("--side-blur", `${sideBlur.value}px`);
  root.style.setProperty("--card-blur", `${cardBlur.value}px`);
  root.style.setProperty(
    "--side-shadow",
    sideShadow.value <= 0 ? "0 0 0 rgba(0,0,0,0)" : `inset -1px 0 0 var(--card-border)`
  );
  root.style.setProperty("--card-shadow-strength", String(cardShadow.value));
  root.style.setProperty("--bg-alpha", String(bgAlpha.value / 100));
  root.style.setProperty("--side-alpha", String(sideAlpha.value / 100));
  root.style.setProperty("--card-alpha", String(cardAlpha.value / 100));

  root.style.setProperty("--motion", String(motion.value));
  root.style.setProperty("--dur", motion.value <= 0.01 ? "0ms" : `${Math.round(150 + 90 * motion.value)}ms`);
  root.style.setProperty("--hover-lift", `${(4 * motion.value).toFixed(2)}px`);
  root.style.setProperty("--hover-scale", `${(1 + 0.016 * motion.value).toFixed(4)}`);

  const img =
    wallKind.value === "image" && wallImageUrl.value ? `url("${wallImageUrl.value}")` : "none";
  root.style.setProperty("--wall-image", img);
  root.style.setProperty("--wall-image-opacity", wallKind.value === "none" ? "0" : "1");

  document.body.classList.toggle("glass-off", !glass.value);
}

export function saveAppearance() {
  localStorage.setItem("ui.theme", themePref.value);
  localStorage.setItem("ui.accent", accent.value);
  localStorage.setItem("ui.customAccent", customAccent.value);
  localStorage.setItem("ui.glass", glass.value ? "on" : "off");
  localStorage.setItem("ui.material", material.value);
  localStorage.setItem("ui.blur", String(blur.value));
  localStorage.setItem("ui.bgAlpha", String(bgAlpha.value));
  localStorage.setItem("ui.sideAlpha", String(sideAlpha.value));
  localStorage.setItem("ui.cardAlpha", String(cardAlpha.value));
  localStorage.setItem("ui.bgBlur", String(bgBlur.value));
  localStorage.setItem("ui.sideBlur", String(sideBlur.value));
  localStorage.setItem("ui.cardBlur", String(cardBlur.value));
  localStorage.setItem("ui.sideShadow", String(sideShadow.value));
  localStorage.setItem("ui.cardShadow", String(cardShadow.value));
  localStorage.setItem("ui.appNameEn", appNameEnglish.value ? "1" : "0");
  localStorage.setItem("ui.appColor", appColorMode.value);
  localStorage.setItem("ui.motion", String(motion.value));
  localStorage.setItem("ui.wallKind", wallKind.value);
  localStorage.setItem("ui.v", UI_VERSION);
}

/* ---------- 主题包（导出/导入 JSON） ---------- */
export interface ThemePack {
  format: string;
  version: number;
  name: string;
  appearance: Record<string, string | number>;
}

export function exportPack(): ThemePack {
  return {
    format: "tallymoment-theme",
    version: 1,
    name: `拾刻主题 ${new Date().toLocaleString("zh-CN")}`,
    appearance: {
      themePref: themePref.value,
      accent: accent.value,
      glass: glass.value ? "on" : "off",
      material: material.value,
      blur: blur.value,
      bgAlpha: bgAlpha.value,
      sideAlpha: sideAlpha.value,
      cardAlpha: cardAlpha.value,
      bgBlur: bgBlur.value,
      sideBlur: sideBlur.value,
      cardBlur: cardBlur.value,
      cardShadow: cardShadow.value,
      appNameEnglish: appNameEnglish.value ? "1" : "0",
      appColorMode: appColorMode.value,
      motion: motion.value,
      wallKind: wallKind.value,
    },
  };
}

export function importPack(pack: ThemePack): string | null {
  if (!pack || pack.format !== "tallymoment-theme" || !pack.appearance) {
    return "不是有效的拾刻主题包";
  }
  const a = pack.appearance;
  const n = (k: string, d: number) => {
    const v = Number(a[k]);
    return Number.isFinite(v) ? v : d;
  };
  themePref.value = (a.themePref as ThemePref) || "dark";
  accent.value = String(a.accent || "indigo");
  glass.value = a.glass !== "off";
  material.value = (a.material as Material) === "liquid" ? "liquid" : "frosted";
  blur.value = n("blur", 26);
  bgAlpha.value = n("bgAlpha", 80);
  sideAlpha.value = n("sideAlpha", 50);
  cardAlpha.value = n("cardAlpha", 100);
  bgBlur.value = n("bgBlur", 18);
  sideBlur.value = n("sideBlur", 26);
  cardBlur.value = n("cardBlur", 26);
  cardShadow.value = n("cardShadow", 1);
  appNameEnglish.value = String(a.appNameEnglish ?? "0") === "1";
  appColorMode.value = (["accent", "iconColor", "icon"].includes(String(a.appColorMode))
    ? (a.appColorMode as AppColorMode)
    : "random");
  motion.value = n("motion", 1);
  wallKind.value = (a.wallKind as WallKind) || "gradient";
  return null;
}

/* ---------- 颜色小工具 ---------- */
export function hexAlpha(hex: string, a: number): string {
  const h = hex.replace("#", "");
  const full = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  const r = parseInt(full.slice(0, 2), 16) || 0;
  const g = parseInt(full.slice(2, 4), 16) || 0;
  const b = parseInt(full.slice(4, 6), 16) || 0;
  return `rgba(${r},${g},${b},${a})`;
}

const clamp = (v: number) => Math.max(0, Math.min(255, Math.round(v)));

function lighten(hex: string, k: number): string {
  const h = hex.replace("#", "");
  const r = parseInt(h.slice(0, 2), 16) || 0;
  const g = parseInt(h.slice(2, 4), 16) || 0;
  const b = parseInt(h.slice(4, 6), 16) || 0;
  return `rgb(${clamp(r + (255 - r) * k)},${clamp(g + (255 - g) * k)},${clamp(b + (255 - b) * k)})`;
}

function darken(hex: string, k: number): string {
  const h = hex.replace("#", "");
  const r = parseInt(h.slice(0, 2), 16) || 0;
  const g = parseInt(h.slice(2, 4), 16) || 0;
  const b = parseInt(h.slice(4, 6), 16) || 0;
  return `rgb(${clamp(r * (1 - k))},${clamp(g * (1 - k))},${clamp(b * (1 - k))})`;
}

/** 强调色是否跟随皮肤（用于个性化页的"跟随皮肤"提示） */
export const accentIsPreset = computed(() => ACCENTS.includes(accent.value));
