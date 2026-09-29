<script setup lang="ts">
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { NSwitch } from "naive-ui";
import Icon from "../components/Icon.vue";
import SettingSlider from "../components/SettingSlider.vue";
import { appsTopN } from "../lib/uiState";
import { onMounted } from "vue";
import {
  accent,
  accentIsPreset,
  applySkin,
  activeSkinKey,
  bgAlpha,
  cardAlpha,
  bgBlur,
  sideBlur,
  cardBlur,
  cardShadow,
  appColorMode,
  exportPack,
  glass,
  importPack,
  material,
  motion,
  sideAlpha,
  skins,
  themePref,
  wallKind,
  wallImageUrl,
  type ThemePack,
} from "../lib/appearance";

const emit = defineEmits<{ (e: "wallpaper-changed"): void }>();

const msg = ref("");
const reminderBgSet = ref(false);
const err = ref("");

const themes: { key: "dark" | "light" | "system"; label: string; icon: string }[] = [
  { key: "dark", label: "深色", icon: "moon" },
  { key: "light", label: "浅色", icon: "sun" },
  { key: "system", label: "跟随系统", icon: "settings" },
];

const accentPresets: { key: string; color: string; label: string }[] = [
  { key: "indigo", color: "#7b84ec", label: "靛蓝" },
  { key: "teal", color: "#58b3c4", label: "青" },
  { key: "green", color: "#6fb59a", label: "薄荷" },
  { key: "violet", color: "#a08fe0", label: "紫藤" },
  { key: "amber", color: "#d3a35e", label: "琥珀" },
  { key: "rose", color: "#d3859b", label: "蔷薇" },
];

const materials: { key: "frosted" | "liquid"; label: string; desc: string }[] = [
  { key: "frosted", label: "毛玻璃", desc: "奶霜质感，文字更清晰" },
  { key: "liquid", label: "液态玻璃", desc: "更薄更透，折射感强，更显壁纸" },
];

const wallKinds: { key: "none" | "gradient" | "image"; label: string }[] = [
  { key: "none", label: "无" },
  { key: "gradient", label: "柔光渐变" },
  { key: "image", label: "本地图片" },
];

const currentSkin = computed(() => activeSkinKey());

/** 取色器：预设色块选中时显示该预设的实际色值，拖动即切到自定义色 */
const pickerValue = computed({
  get: () => {
    const hit = accentPresets.find((a) => a.key === accent.value);
    return hit ? hit.color : accent.value;
  },
  set: (v: string) => (accent.value = v),
});

const accentLabel = computed(() =>
  accentIsPreset.value
    ? accentPresets.find((a) => a.key === accent.value)?.label ?? "预设"
    : "自定义"
);

function pickSkin(key: string) {
  const s = skins.find((x) => x.key === key);
  if (s) {
    applySkin(s);
    msg.value = `已应用皮肤「${s.label}」`;
    err.value = "";
  }
}

function randomAccent() {
  const list = accentPresets.map((a) => a.color);
  accent.value = list[Math.floor(Math.random() * list.length)];
  msg.value = "已随机换一个强调色";
}

async function pickWallpaper() {
  msg.value = "";
  err.value = "";
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择壁纸图片",
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    await invoke("wallpaper_set", { path: picked });
    wallKind.value = "image";
    emit("wallpaper-changed");
    msg.value = "壁纸已设置（只保存在本机数据目录）";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function clearWallpaper() {
  msg.value = "";
  err.value = "";
  try {
    await invoke("wallpaper_clear");
    if (wallImageUrl.value) URL.revokeObjectURL(wallImageUrl.value);
    wallImageUrl.value = null;
    wallKind.value = "gradient";
    msg.value = "已移除壁纸，回到柔光渐变";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function exportTheme() {
  msg.value = "";
  err.value = "";
  try {
    const picked = await save({
      title: "导出主题包",
      defaultPath: "tallymoment-theme.json",
      filters: [{ name: "主题包", extensions: ["json"] }],
    });
    if (!picked) return;
    const json = JSON.stringify(exportPack(), null, 2);
    await invoke("theme_export", { path: picked, json });
    msg.value = "主题包已导出（壁纸图片不包含在内）";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function importTheme() {
  msg.value = "";
  err.value = "";
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择主题包",
      filters: [{ name: "主题包", extensions: ["json"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    const txt = await invoke<string>("theme_import", { path: picked });
    const problem = importPack(JSON.parse(txt) as ThemePack);
    if (problem) {
      err.value = problem;
      return;
    }
    emit("wallpaper-changed");
    msg.value = "主题包已导入并应用";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function pickReminderBg() {
  msg.value = "";
  err.value = "";
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "选择全屏提醒背景图",
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    await invoke("reminder_bg_set", { path: picked });
    reminderBgSet.value = true;
    msg.value = "全屏提醒背景已设置";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

async function clearReminderBg() {
  msg.value = "";
  err.value = "";
  try {
    await invoke("reminder_bg_clear");
    reminderBgSet.value = false;
    msg.value = "已移除，回到默认柔光背景";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}

void invoke<{ mime: string; data: string } | null>("reminder_bg_get")
  .then((w) => (reminderBgSet.value = !!w))
  .catch(() => (reminderBgSet.value = false));


/* ---------- 原子岛 ---------- */
const islandOn = ref(false);
async function setIsland(v: boolean) {
  try {
    await invoke("island_set_enabled", { enabled: v });
    msg.value = v ? "原子岛已开启" : "原子岛已关闭";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
async function resetIslandPos() {
  try {
    await invoke("island_reset_pos");
    msg.value = "原子岛位置已重置";
  } catch (e) {
    err.value = String(e).replace(/^.*Error: /, "");
  }
}
onMounted(async () => {
  try {
    const enabled = await invoke<boolean>("island_get_enabled");
    islandOn.value = enabled;
  } catch {
    islandOn.value = false;
  }
});
</script>

<template>
  <div class="page">
    <header class="phead">
      <h1>个性化</h1>
      <span class="sub">皮肤预设 · 壁纸 · 玻璃材质 · 透明度 · 动效</span>
    </header>

    <!-- 显示（原在设置页，按用户要求移到这里，放在皮肤预设之前） -->
    <div class="glass-card card">
      <h2>显示</h2>
      <div class="row col">
        <div class="rlabel">
          <p class="rt">应用配色</p>
          <p class="rd">
            随机色更易区分；跟随强调色更整体；按图标取色＝从程序图标提取主色；显示应用图标＝直接显示 exe 图标
          </p>
        </div>
        <div class="seg grid2">
          <button
            class="seg-item"
            :class="{ active: appColorMode === 'random' }"
            @click="appColorMode = 'random'"
          >
            随机色
          </button>
          <button
            class="seg-item"
            :class="{ active: appColorMode === 'accent' }"
            @click="appColorMode = 'accent'"
          >
            跟随强调色
          </button>
          <button
            class="seg-item"
            :class="{ active: appColorMode === 'iconColor' }"
            @click="appColorMode = 'iconColor'"
          >
            按图标取色
          </button>
          <button
            class="seg-item"
            :class="{ active: appColorMode === 'icon' }"
            @click="appColorMode = 'icon'"
          >
            显示应用图标
          </button>
        </div>
      </div>
      <SettingSlider
        v-model="appsTopN"
        label="应用排行显示条数"
        desc="5 ~ 20 条，默认 10 条"
        :min="5"
        :max="20"
        suffix=" 条"
      />
    </div>

    <!-- 原子岛（常驻胶囊） -->
    <div class="glass-card card">
      <h2>原子岛</h2>
      <div class="row">
        <span>启用常驻胶囊</span>
        <NSwitch v-model:value="islandOn" size="small" @update:value="setIsland" />
      </div>
      <p class="rd more">
        屏幕常驻小条：当前专注 / 下个任务倒计时 / 今日完成数，点击展开待办并可直接勾掉；
        拖动胶囊可挪位置，位置会记住。常驻约多占 40~80MB 内存（一个 WebView 窗口）。
      </p>
      <div class="acts" style="display: flex; gap: 8px">
        <button class="btn" @click="resetIslandPos">位置重置（回屏幕顶部居中）</button>
      </div>
      <p v-if="msg" class="okline">{{ msg }}</p>
    </div>

    <!-- 皮肤预设 -->
    <div class="glass-card card">
      <h2>皮肤预设</h2>
      <div class="skins">
        <button
          v-for="s in skins"
          :key="s.key"
          class="skin"
          :class="{ active: currentSkin === s.key }"
          :title="s.label"
          @click="pickSkin(s.key)"
        >
          <span class="preview" :style="{ background: s.swatch[0] }">
            <span class="pv-card" :style="{ background: s.swatch[1] }">
              <span class="pv-bar" :style="{ background: s.swatch[2] }"></span>
              <span class="pv-bar short" :style="{ background: s.swatch[2], opacity: 0.6 }"></span>
            </span>
          </span>
          <span class="sname">{{ s.label }}</span>
          <span v-if="currentSkin === s.key" class="tick"><Icon name="checklist" :size="12" /></span>
        </button>
      </div>
      <p class="rd more">预设会一次性设定主题、强调色、材质与透明度；之后你仍然可以单独微调任何一项。</p>
    </div>

    <!-- 强调色 -->
    <div class="glass-card card">
      <h2>强调色</h2>
      <div class="accents">
        <button
          v-for="a in accentPresets"
          :key="a.key"
          class="swatch"
          :class="{ active: accent === a.key }"
          :style="{ background: a.color }"
          :title="a.label"
          @click="accent = a.key"
        ></button>
        <label class="picker" :title="`自定义颜色（当前：${accentLabel}）`">
          <input v-model="pickerValue" type="color" />
        </label>
        <button class="ghost" @click="randomAccent">随机</button>
        <button class="ghost" @click="accent = 'indigo'">恢复默认</button>
        <span class="cur">{{ accentLabel }}</span>
      </div>
    </div>

    <!-- 主题与壁纸 -->
    <div class="glass-card card">
      <h2>主题与壁纸</h2>
      <div class="row">
        <div class="rlabel">
          <p class="rt">明暗主题</p>
          <p class="rd">浅色主题下图表与图标也已适配</p>
        </div>
        <div class="seg">
          <button
            v-for="t in themes"
            :key="t.key"
            class="seg-item"
            :class="{ active: themePref === t.key }"
            @click="themePref = t.key"
          >
            <Icon :name="t.icon" :size="15" />
            <span>{{ t.label }}</span>
          </button>
        </div>
      </div>

      <div class="row">
        <div class="rlabel">
          <p class="rt">壁纸</p>
          <p class="rd">玻璃效果需要有"东西"可糊，柔光渐变是最省性能的选择</p>
        </div>
        <div class="seg">
          <button
            v-for="w in wallKinds"
            :key="w.key"
            class="seg-item"
            :class="{ active: wallKind === w.key }"
            @click="wallKind = w.key"
          >
            {{ w.label }}
          </button>
        </div>
      </div>

      <div class="row">
        <div class="rlabel">
          <p class="rt">本地图片</p>
          <p class="rd">从本机选择一张图作为底衬（只存本机数据目录，不上传）</p>
        </div>
        <div class="acts">
          <button class="ghost" @click="pickWallpaper">选择图片</button>
          <button class="ghost danger" @click="clearWallpaper">移除图片</button>
        </div>
      </div>
    </div>

    <!-- 玻璃效果：所有"有多透"的控制收在同一组 -->
    <div class="glass-card card">
      <h2>玻璃效果</h2>

      <div class="row">
        <div class="rlabel">
          <p class="rt">毛玻璃总开关</p>
          <p class="rd">关闭后界面变为不透明实底（低配设备可关）</p>
        </div>
        <NSwitch :value="glass" @update:value="(v: boolean) => (glass = v)" />
      </div>

      <div class="materials">
        <button
          v-for="m in materials"
          :key="m.key"
          class="mat"
          :class="{ active: material === m.key }"
          @click="material = m.key"
        >
          <span class="mat-pv" :class="m.key"></span>
          <span class="sname">{{ m.label }}</span>
          <span class="sdesc">{{ m.desc }}</span>
        </button>
      </div>

    </div>

    <!-- 背景 -->
    <div class="glass-card card">
      <h2>背景</h2>
      <SettingSlider v-model="bgBlur" label="背景模糊" desc="0 ~ 60px：柔光/壁纸的朦胧程度" :min="0" :max="60" suffix="px" />
      <SettingSlider v-model="bgAlpha" label="背景不透明度" desc="30% ~ 100%，越低桌面越透" :min="30" :max="100" suffix="%" />
    </div>

    <!-- 侧边栏 -->
    <div class="glass-card card">
      <h2>侧边栏</h2>
      <SettingSlider v-model="sideBlur" label="侧边栏亚克力模糊" desc="0 ~ 60px" :min="0" :max="60" suffix="px" />
      <SettingSlider v-model="sideAlpha" label="侧边栏不透明度" desc="0% ~ 100%" :min="0" :max="100" suffix="%" />
    </div>

    <!-- 卡片 -->
    <div class="glass-card card">
      <h2>卡片</h2>
      <SettingSlider v-model="cardBlur" label="卡片亚克力模糊" desc="0 ~ 60px" :min="0" :max="60" suffix="px" />
      <SettingSlider v-model="cardAlpha" label="卡片不透明度" desc="50% ~ 100%" :min="50" :max="100" suffix="%" />
      <SettingSlider v-model="cardShadow" label="卡片阴影强度" desc="0 = 无阴影（扁平）；1 = 标准；越大浮起感越强" :min="0" :max="2" :step="0.1" suffix="×" />
    </div>

    <!-- 全屏提醒背景 -->
    <div class="glass-card card">
      <h2>全屏提醒背景</h2>
      <div class="row">
        <div class="rlabel">
          <p class="rt">自定义图片</p>
          <p class="rd">全屏休息提醒时铺满屏幕；不设则用默认的柔光背景</p>
        </div>
        <div class="acts">
          <button class="ghost" @click="pickReminderBg">选择图片</button>
          <button class="ghost danger" @click="clearReminderBg">移除</button>
        </div>
      </div>
      <p v-if="reminderBgSet" class="ok">已设置（存本机数据目录，不上传）</p>
    </div>

    <!-- 动效 -->
    <div class="glass-card card">
      <h2>动效</h2>
      <SettingSlider
        v-model="motion"
        label="动效强度"
        desc="0 = 关闭动画；1 = 标准；越大悬停放大与上浮越明显"
        :min="0"
        :max="1.6"
        :step="0.1"
        suffix="×"
      />
      <p class="rd more">
        现在把鼠标移到任意卡片上试试：卡片会上浮并轻微放大、描边变成强调色。强度拉到 0 就是完全静态。
      </p>
    </div>

    <!-- 主题包 -->
    <div class="glass-card card">
      <h2>主题包（本机文件）</h2>
      <div class="acts">
        <button class="ghost" @click="exportTheme">导出主题包</button>
        <button class="ghost" @click="importTheme">导入主题包</button>
      </div>
      <p class="rd more">
        导出为一份 JSON（皮肤/材质/透明度/动效参数），可在别的机器上导入复现同一套观感。壁纸图片不包含在内。
      </p>
    </div>

    <p v-if="msg" class="ok">{{ msg }}</p>
    <p v-if="err" class="err">{{ err }}</p>
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

/* 皮肤预设 */
.skins {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
  gap: 10px;
}

.skin {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 6px;
  align-items: stretch;
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-md);
  padding: 8px;
  cursor: pointer;
  font-family: inherit;
  transition: transform var(--dur), border-color var(--dur), box-shadow var(--dur);
}

.skin:hover {
  transform: translateY(calc(-2px * var(--motion))) scale(calc(1 + 0.02 * var(--motion)));
  border-color: var(--accent-border);
  box-shadow: var(--shadow-soft);
}

.skin.active {
  border-color: var(--accent-border);
  box-shadow: 0 0 0 2px var(--accent-soft) inset;
}

.preview {
  display: block;
  height: 46px;
  border-radius: var(--r-sm);
  padding: 5px;
}

.pv-card {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 4px;
  height: 100%;
  border-radius: 5px;
  padding: 0 6px;
}

.pv-bar {
  display: block;
  height: 3px;
  border-radius: 2px;
}

.pv-bar.short {
  width: 60%;
}

.sname {
  font-size: 12px;
  color: var(--text);
}

.sdesc {
  font-size: 11px;
  color: var(--text-muted);
}

.tick {
  position: absolute;
  top: 6px;
  right: 6px;
  color: var(--accent-text);
  background: var(--accent-soft);
  border-radius: var(--r-full);
  padding: 2px;
  display: inline-flex;
}

/* 强调色 */
.accents {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.swatch {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  transition: transform var(--dur), box-shadow var(--dur);
}

.swatch:hover {
  transform: scale(calc(1 + 0.12 * var(--motion)));
}

.swatch.active {
  border-color: var(--text);
  box-shadow: 0 0 0 3px var(--accent-soft);
}

.picker {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  border: 1px dashed var(--border-strong);
  overflow: hidden;
  display: inline-flex;
  cursor: pointer;
}

.picker input {
  width: 200%;
  height: 200%;
  margin: -25%;
  border: 0;
  padding: 0;
  background: transparent;
  cursor: pointer;
}

.cur {
  font-size: 11.5px;
  color: var(--text-faint);
}

.ghost {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-sm);
  font-size: 12px;
  font-family: inherit;
  padding: 6px 12px;
  cursor: pointer;
  transition: background var(--dur), color var(--dur);
}

.ghost:hover {
  background: var(--surface-hover);
  color: var(--text);
}

.ghost.danger:hover {
  color: var(--danger);
  border-color: var(--danger);
}

.acts {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 8px 0;
}

.rlabel .rt {
  margin: 0 0 2px;
  font-size: 13.5px;
  color: var(--text);
}

.rlabel .rd {
  margin: 0;
  font-size: 11.5px;
  color: var(--text-muted);
}

.seg {
  display: inline-flex;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  padding: 4px;
  background: var(--surface);
  flex-wrap: wrap;
}

/* 四个配色模式：上两个、下两个，等宽对齐（窗口模式下也不歪） */
.seg.grid2 {
  display: grid;
  grid-template-columns: repeat(2, minmax(140px, 1fr));
  gap: 6px;
}

.seg.grid2 .seg-item {
  justify-content: center;
}

.seg-item {
  display: flex;
  align-items: center;
  gap: 6px;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 12.5px;
  font-family: inherit;
  padding: 6px 12px;
  border-radius: var(--r-sm);
  cursor: pointer;
}

.seg-item.active {
  color: var(--accent-text);
  background: var(--accent-soft);
}

/* 材质双卡 */
.materials {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin: 6px 0 4px;
}

.mat {
  display: flex;
  flex-direction: column;
  gap: 4px;
  text-align: left;
  border: 1px solid var(--border);
  background: var(--surface);
  border-radius: var(--r-md);
  padding: 10px;
  cursor: pointer;
  font-family: inherit;
  transition: transform var(--dur), border-color var(--dur);
}

.mat:hover {
  transform: translateY(calc(-2px * var(--motion)));
}

.mat.active {
  border-color: var(--accent-border);
  box-shadow: 0 0 0 2px var(--accent-soft) inset;
}

.mat-pv {
  display: block;
  height: 42px;
  border-radius: var(--r-sm);
  background-image: linear-gradient(
    100deg,
    #ff9966,
    #ffd166,
    #6fb59a,
    #58b3c4,
    #a08fe0
  );
}

.mat-pv.frosted {
  backdrop-filter: blur(8px) saturate(1.3) brightness(1.08);
  opacity: 0.85;
}

.mat-pv.liquid {
  backdrop-filter: blur(2px) saturate(1.8) brightness(1.05) contrast(1.04);
  border: 1px solid color-mix(in srgb, var(--text) 30%, transparent);
}

.rd.more {
  margin: 6px 0 0;
  font-size: 11.5px;
  line-height: 1.7;
  color: var(--text-faint);
}

.ok {
  margin: 0;
  font-size: 12px;
  color: var(--good);
}

.err {
  margin: 0;
  font-size: 12px;
  color: var(--danger);
}
</style>
