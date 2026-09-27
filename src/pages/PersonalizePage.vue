<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NSlider, NSwitch } from "naive-ui";
import Icon from "../components/Icon.vue";

type ThemePref = "dark" | "light" | "system";

const props = defineProps<{
  themePref: ThemePref;
  accent: string;
  glass: boolean;
  blur: number;
  bgAlpha: number;
}>();

const emit = defineEmits<{
  (e: "update:themePref", v: ThemePref): void;
  (e: "update:accent", v: string): void;
  (e: "update:glass", v: boolean): void;
  (e: "update:blur", v: number): void;
  (e: "update:bgAlpha", v: number): void;
}>();

const themes: { key: ThemePref; label: string; icon: string }[] = [
  { key: "dark", label: "深色", icon: "moon" },
  { key: "light", label: "浅色", icon: "sun" },
  { key: "system", label: "跟随系统", icon: "settings" },
];

const accents: { key: string; color: string; label: string }[] = [
  { key: "indigo", color: "#7b84ec", label: "靛蓝" },
  { key: "teal", color: "#58b3c4", label: "青" },
  { key: "green", color: "#6fb59a", label: "薄荷" },
  { key: "violet", color: "#a08fe0", label: "紫藤" },
  { key: "amber", color: "#d3a35e", label: "琥珀" },
  { key: "rose", color: "#d3859b", label: "蔷薇" },
];

/* ---------- 窗口特效（持久化在后端 settings 表） ---------- */
const effectKind = ref("acrylic");
const effectMsg = ref("");
/** 本地即时值：拖动中先反馈，松手（dragend）与 200ms 防抖后落盘
 *  注意：naive-ui 的 Slider 只发 update:value / dragend，没有 change 事件 */
const localBlur = ref(props.blur);
const localAlpha = ref(props.bgAlpha);
let blurTimer: number | undefined;
let alphaTimer: number | undefined;

function onBlurInput(v: number) {
  localBlur.value = v;
  if (blurTimer) clearTimeout(blurTimer);
  blurTimer = window.setTimeout(() => emit("update:blur", localBlur.value), 200);
}

function onAlphaInput(v: number) {
  localAlpha.value = v;
  if (alphaTimer) clearTimeout(alphaTimer);
  alphaTimer = window.setTimeout(() => emit("update:bgAlpha", localAlpha.value), 200);
}

function commitBlur() {
  if (blurTimer) clearTimeout(blurTimer);
  emit("update:blur", localBlur.value);
}

function commitAlpha() {
  if (alphaTimer) clearTimeout(alphaTimer);
  emit("update:bgAlpha", localAlpha.value);
}

const effects: { key: string; label: string; desc: string }[] = [
  { key: "acrylic", label: "亚克力", desc: "Windows 11 原生，最通透（窗口失焦时系统会自动减淡）" },
  { key: "mica", label: "云母", desc: "更含蓄的桌面取样，适合长时间看" },
  { key: "blur", label: "经典模糊", desc: "旧版 Aero 模糊，失焦也保留" },
  { key: "none", label: "关闭", desc: "关闭窗口特效，只用界面内部的模糊" },
];

async function pickEffect(kind: string) {
  effectMsg.value = "";
  try {
    await invoke("set_window_effect", { kind });
    effectKind.value = kind;
    effectMsg.value = kind === "none" ? "已关闭窗口特效" : "窗口特效已切换";
  } catch (e) {
    effectMsg.value = String(e).replace(/^.*Error: /, "");
  }
}

onMounted(async () => {
  try {
    effectKind.value = await invoke<string>("window_effect_get");
  } catch {
    /* 后端未就绪时按默认 */
  }
});
</script>

<template>
  <div class="page">
    <header class="phead">
      <h1>个性化</h1>
      <span class="sub">外观与毛玻璃效果</span>
    </header>

    <div class="glass-card card">
      <h2>主题</h2>
      <div class="seg">
        <button
          v-for="t in themes"
          :key="t.key"
          class="seg-item"
          :class="{ active: themePref === t.key }"
          @click="emit('update:themePref', t.key)"
        >
          <Icon :name="t.icon" :size="16" />
          <span>{{ t.label }}</span>
        </button>
      </div>
    </div>

    <div class="glass-card card">
      <h2>强调色</h2>
      <div class="accents">
        <button
          v-for="a in accents"
          :key="a.key"
          class="swatch"
          :class="{ active: accent === a.key }"
          :style="{ background: a.color }"
          :title="a.label"
          @click="emit('update:accent', a.key)"
        ></button>
      </div>
    </div>

    <div class="glass-card card">
      <h2>毛玻璃</h2>

      <div class="row">
        <div class="rlabel">
          <p class="rt">毛玻璃</p>
          <p class="rd">半透明卡片与侧栏（低配设备可关闭）</p>
        </div>
        <NSwitch :value="glass" @update:value="(v: boolean) => emit('update:glass', v)" />
      </div>

      <div class="row col">
        <div class="rlabel">
          <p class="rt">窗口特效</p>
          <p class="rd">让桌面透过窗口形成玻璃质感</p>
        </div>
        <div class="seg wrap">
          <button
            v-for="e in effects"
            :key="e.key"
            class="seg-item"
            :class="{ active: effectKind === e.key }"
            :title="e.desc"
            @click="pickEffect(e.key)"
          >
            {{ e.label }}
          </button>
        </div>
        <p class="rd">{{ effects.find((e) => e.key === effectKind)?.desc }}</p>
      </div>

      <div class="row col">
        <div class="rlabel">
          <p class="rt">玻璃模糊</p>
          <p class="rd">0 ~ 40px，越大越朦胧</p>
        </div>
        <NSlider
          :value="localBlur"
          :min="0"
          :max="40"
          :step="1"
          :format-tooltip="(v: number) => v + 'px'"
          style="max-width: 320px"
          @update:value="onBlurInput"
          @dragend="commitBlur"
        />
      </div>

      <div class="row col">
        <div class="rlabel">
          <p class="rt">界面底色不透明度</p>
          <p class="rd">30% ~ 100%，越低桌面越透</p>
        </div>
        <NSlider
          :value="localAlpha"
          :min="30"
          :max="100"
          :step="2"
          :format-tooltip="(v: number) => v + '%'"
          style="max-width: 320px"
          @update:value="onAlphaInput"
          @dragend="commitAlpha"
        />
      </div>

      <p v-if="effectMsg" class="hint">{{ effectMsg }}</p>
      <p class="rd more">
        提示：窗口特效由 Windows 绘制，需要窗口未最大化且显卡驱动正常；若出现黑底或闪烁，切到「关闭」即可。
      </p>
    </div>
  </div>
</template>

<style scoped>
.page {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.phead {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.phead h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
}

.sub {
  font-size: 12px;
  color: var(--text-muted);
}

.card {
  padding: 16px 18px;
}

.card h2 {
  margin: 0 0 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

.seg {
  display: inline-flex;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  padding: 4px;
  background: var(--surface);
}

.seg.wrap {
  flex-wrap: wrap;
}

.seg-item {
  display: flex;
  align-items: center;
  gap: 6px;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 13px;
  font-family: inherit;
  padding: 7px 14px;
  border-radius: var(--r-sm);
  cursor: pointer;
}

.seg-item.active {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.accents {
  display: flex;
  gap: 10px;
}

.swatch {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  transition: transform 0.12s, box-shadow 0.12s;
}

.swatch:hover {
  transform: scale(1.1);
}

.swatch.active {
  border-color: var(--text);
  box-shadow: 0 0 0 3px var(--accent-soft);
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 8px 0;
}

.row.col {
  flex-direction: column;
  align-items: stretch;
  gap: 8px;
}

.rlabel .rt {
  margin: 0 0 2px;
  font-size: 14px;
  color: var(--text);
}

.rlabel .rd {
  margin: 0;
  font-size: 12px;
  color: var(--text-muted);
}

.hint {
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--good);
}

.rd.more {
  margin-top: 10px;
  font-size: 12px;
  color: var(--text-faint);
}

button {
  font-family: inherit;
}
</style>
