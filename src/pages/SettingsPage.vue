<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { NSwitch } from "naive-ui";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import Icon from "../components/Icon.vue";

type ThemePref = "dark" | "light" | "system";

const props = defineProps<{
  themePref: ThemePref;
  accent: string;
  glass: boolean;
  petVisible: boolean;
  brandLang: string;
}>();

const emit = defineEmits<{
  (e: "update:themePref", v: ThemePref): void;
  (e: "update:accent", v: string): void;
  (e: "update:glass", v: boolean): void;
  (e: "update:petVisible", v: boolean): void;
  (e: "toggleGlass"): void;
  (e: "togglePet"): void;
  (e: "update:brandLang", v: string): void;
}>();

const themes: { key: ThemePref; label: string; icon: string }[] = [
  { key: "dark", label: "深色", icon: "moon" },
  { key: "light", label: "浅色", icon: "sun" },
  { key: "system", label: "跟随系统", icon: "settings" },
];

const autoStart = ref(false);

async function toggleAutoStart(v: boolean) {
  try {
    if (v) await enable();
    else await disable();
    autoStart.value = v;
  } catch {
    autoStart.value = false;
  }
}

onMounted(async () => {
  try {
    autoStart.value = await isEnabled();
  } catch {
    autoStart.value = false;
  }
});

const brandLang = computed({
  get: () => props.brandLang as string,
  set: (v: string) => emit("update:brandLang", v),
});

const brandOptions = [
  { key: "zh", label: "中文 · 拾刻" },
  { key: "en", label: "英文 · TallyMoment" },
  { key: "both", label: "双语 · 拾刻 TallyMoment" },
];

const accents: { key: string; color: string; label: string }[] = [
  { key: "indigo", color: "#7b84ec", label: "靛蓝" },
  { key: "teal", color: "#58b3c4", label: "青" },
  { key: "green", color: "#6fb59a", label: "薄荷" },
  { key: "violet", color: "#a08fe0", label: "紫藤" },
  { key: "amber", color: "#d3a35e", label: "琥珀" },
  { key: "rose", color: "#d3859b", label: "蔷薇" },
];
</script>

<template>
  <div class="page">
    <header class="phead">
      <h1>个性化</h1>
      <span class="sub">外观与桌宠</span>
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
      <h2>品牌语言</h2>
      <div class="seg">
        <button
          v-for="b in brandOptions"
          :key="b.key"
          class="seg-item"
          :class="{ active: brandLang === b.key }"
          @click="brandLang = b.key"
        >
          {{ b.label }}
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
      <h2>效果</h2>
      <div class="row">
        <div>
          <p class="rt">毛玻璃</p>
          <p class="rd">半透明卡片与侧栏（低配设备可关闭）</p>
        </div>
        <NSwitch :value="glass" @update:value="() => emit('toggleGlass')" />
      </div>
    </div>

    <div class="glass-card card">
      <h2>系统</h2>
      <div class="row">
        <div>
          <p class="rt">开机自启动</p>
          <p class="rd">登录 Windows 后自动在后台运行并开始记录</p>
        </div>
        <NSwitch :value="autoStart" @update:value="toggleAutoStart" />
      </div>
    </div>

    <div class="glass-card card">
      <h2>桌宠</h2>
      <div class="row">
        <div>
          <p class="rt">显示桌宠</p>
          <p class="rd">屏幕右下角的猫，也可以在托盘菜单开关</p>
        </div>
        <NSwitch :value="petVisible" @update:value="() => emit('togglePet')" />
      </div>
      <p class="rd more">更多桌宠设置（缩放/透明度/模型导入）将在桌宠增强批次提供</p>
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
}

.rt {
  margin: 0 0 2px;
  font-size: 14px;
  color: var(--text);
}

.rd {
  margin: 0;
  font-size: 12px;
  color: var(--text-muted);
}

.rd.more {
  margin-top: 10px;
  color: var(--text-faint);
}

button {
  font-family: inherit;
}
</style>
