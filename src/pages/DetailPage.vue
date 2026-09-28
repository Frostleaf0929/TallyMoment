<script setup lang="ts">
import { computed, onMounted, ref, watch, watchEffect } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NInput } from "naive-ui";
import type { AppUsage, RangeReport } from "../types";
import { fmtDuration } from "../lib/format";
import { colorFor } from "../lib/colors";
import { accentColor } from "../lib/chartColors";
import { appColorMode, appNameEnglish } from "../lib/appearance";
import { iconColor, iconUrl, requestIcons } from "../lib/appIcons";
import { navIntent } from "../lib/uiState";
import BallLoader from "../components/BallLoader.vue";
import DayDetail from "../components/DayDetail.vue";
import RangeCard from "../components/RangeCard.vue";
import CalendarHeat from "../components/CalendarHeat.vue";
import DayBars from "../components/DayBars.vue";
import HourlyChart from "../components/HourlyChart.vue";

/**
 * 详细页：**以应用为主**（对标 Tai 的详细）
 * 时间范围固定在"当前"的几种相对区间（今天/本周/本月/本年/全部），不提供任意日期选择
 */
type RangeKey = "day" | "week" | "month" | "year" | "all";

/** 页面内两个视角：时间（按应用看时长）/ 事项（某天的任务与记录对照） */
const view = ref<"time" | "items">("time");
const itemDate = ref<number>(Date.now());
const itemDateStr = computed(() => ymd(new Date(itemDate.value)));
const itemRange = ref<RangeKey>("day");
const itemFrom = ref("");
const itemTo = ref("");

/** 事项视角里选某一天 */
function onPickItemDay(date: string) {
  itemDate.value = new Date(`${date}T00:00:00`).getTime();
}

const rangeKey = ref<RangeKey>("week");
const loading = ref(false);
const report = ref<RangeReport | null>(null);
const selectedApp = ref<string | null>(null);
const keyword = ref("");
const err = ref("");
const fallbackNote = ref("");


function ymd(d: Date): string {
  const m = `${d.getMonth() + 1}`.padStart(2, "0");
  const day = `${d.getDate()}`.padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

/** 相对区间：天=今天、周=本周（周一起）、月=本月、年=本年、总共=有记录以来 */
function rangeOf(k: RangeKey): { from: string; to: string } {
  const now = new Date();
  const today = ymd(now);
  if (k === "day") return { from: today, to: today };
  if (k === "week") {
    const dow = (now.getDay() + 6) % 7;
    const start = new Date(now.getFullYear(), now.getMonth(), now.getDate() - dow);
    const end = new Date(start.getFullYear(), start.getMonth(), start.getDate() + 6);
    return { from: ymd(start), to: ymd(end) };
  }
  if (k === "month") {
    return {
      from: ymd(new Date(now.getFullYear(), now.getMonth(), 1)),
      to: ymd(new Date(now.getFullYear(), now.getMonth() + 1, 0)),
    };
  }
  if (k === "year") {
    return { from: `${now.getFullYear()}-01-01`, to: `${now.getFullYear()}-12-31` };
  }
  return { from: "2000-01-01", to: today };
}

async function load(autoFallback = false) {
  const { from, to } = rangeOf(rangeKey.value);
  loading.value = true;
  err.value = "";
  try {
    const r = await invoke<RangeReport>("range_report", {
      from,
      to,
      appName: selectedApp.value,
    });
    report.value = r;
    // 跳转过来时若该范围没有记录，自动退到"总共"，避免看到一片空白
    if (autoFallback && r.totalSeconds === 0 && rangeKey.value !== "all") {
      rangeKey.value = "all";
      fallbackNote.value = "该时间范围内没有记录，已自动切到「总共」";
      await load(false);
      return;
    }
  } catch (e) {
    report.value = null;
    err.value = String(e).replace(/^.*Error: /, "");
  } finally {
    loading.value = false;
  }
}

function pickRange(k: RangeKey) {
  rangeKey.value = k;
  fallbackNote.value = "";
  void load(false);
}

function pickApp(name: string | null) {
  selectedApp.value = selectedApp.value === name ? null : name;
  fallbackNote.value = "";
  void load(false);
}

const appLabel = (a: AppUsage) => (appNameEnglish.value ? a.name.replace(/\.exe$/i, "") : a.displayName);
const appColor = (a: AppUsage) => {
  if (appColorMode.value === "accent") return accentColor();
  if (appColorMode.value === "iconColor") return iconColor(a.name) || colorFor(a.name);
  return colorFor(a.name);
};

const maxSeconds = computed(() => Math.max(1, ...(report.value?.apps ?? []).map((a) => a.seconds)));

const filteredApps = computed(() => {
  const k = keyword.value.trim().toLowerCase();
  const list = report.value?.apps ?? [];
  if (!k) return list;
  return list.filter((a) => appLabel(a).toLowerCase().includes(k) || a.name.toLowerCase().includes(k));
});

const current = computed(() => {
  if (!selectedApp.value) return null;
  const list = report.value?.apps ?? [];
  return list.find((a) => a.name === selectedApp.value) ?? null;
});

const trend = computed(() => {
  const b = report.value?.buckets ?? [];
  return {
    labels: b.map((x) => x.label),
    series: [
      {
        name: "使用时长",
        color: selectedApp.value ? accentColor() : "var(--accent)",
        data: b.map((x) => Math.round(x.seconds / 60)),
      },
    ],
    interval: b.length > 16 ? Math.ceil(b.length / 12) - 1 : 0,
  };
});

onMounted(() => void load(false));

// 图标模式下批量取图标
watchEffect(() => {
  if (appColorMode.value === "icon" || appColorMode.value === "iconColor") {
    requestIcons(filteredApps.value.map((a) => a.name));
  }
});

// 从今日页/排行跳转过来：选中该应用，若当前范围没数据自动退到"总共"
watch(navIntent, (n) => {
  if (!n) return;
  if (n.view === "items") {
    view.value = "items";
    if (n.date) itemDate.value = new Date(`${n.date}T00:00:00`).getTime();
    return;
  }
  if (n.view !== "app") return;
  view.value = "time";
  selectedApp.value = n.app ?? null;
  void load(true);
});
</script>

<template>
  <div class="detail">
    <header class="head">
      <h1>详细</h1>
      <span class="sub">按应用查看 · 选中左侧应用可下钻</span>
    </header>

    <div class="viewseg">
      <button class="vbtn" :class="{ active: view === 'time' }" @click="view = 'time'">应用</button>
      <button class="vbtn" :class="{ active: view === 'items' }" @click="view = 'items'">事项</button>
    </div>

    <!-- 事项视角：直接复用某天对照详情 -->
    <template v-if="view === 'items'">
      <!-- 与「详细 · 应用」同款的 时间范围 卡（后续周/月/年/总共的聚合在这里接） -->
      <section class="glass-card rangecard">
        <RangeCard
          v-model="itemRange"
          :from="itemFrom"
          :to="itemTo"
          note="周 / 月 / 年 / 总共的范围聚合后续接上；现在先在左侧月历点选某一天查看"
        />
      </section>
      <section class="glass-card itemsview">
        <div class="itemshead">
          <h2>事项与记录</h2>
          <span class="itemsday">{{ itemDateStr }}</span>
        </div>
        <div class="itemsgrid">
          <CalendarHeat
            compact
            :selected="itemDateStr"
            @pick="onPickItemDay"
            @locate="onPickItemDay"
          />
          <div class="itemsdetail">
            <DayDetail :date="itemDateStr" />
          </div>
        </div>
      </section>
    </template>

    <div v-if="view === 'time'" class="split">
      <!-- 应用列表（主入口）：外层只负责撑高，内层绝对定位贴合 → 底部与右列对齐且不撑长页面 -->
      <div class="applist-wrap">
      <aside class="glass-card applist">
        <div class="listhead">
          <h2>应用</h2>
          <NInput v-model:value="keyword" size="small" placeholder="搜索" clearable style="width: 110px" />
        </div>
        <div class="listbody">
          <button
            v-for="a in filteredApps"
            :key="a.name"
            class="approw"
            :class="{ active: selectedApp === a.name }"
            :title="appLabel(a)"
            @click="pickApp(a.name)"
          >
            <img
              v-if="appColorMode === 'icon' && iconUrl(a.name)"
              class="aicon"
              :src="iconUrl(a.name)"
              alt=""
              draggable="false"
            />
            <span v-else class="adot" :style="{ background: appColor(a) }"></span>
            <span class="aname">{{ appLabel(a) }}</span>
            <span class="atime">{{ fmtDuration(a.seconds) }}</span>
            <span class="abar">
              <i :style="{ width: (a.seconds / maxSeconds) * 100 + '%', background: appColor(a) }"></i>
            </span>
          </button>
          <p v-if="!filteredApps.length" class="empty">这个范围没有记录</p>
        </div>
      </aside>
      </div>

      <div class="right">
        <!-- 时间范围卡（与事项视角共用同一组件） -->
        <section class="glass-card rangecard">
          <RangeCard
            :model-value="rangeKey"
            :from="report?.from"
            :to="report?.to"
            :note="fallbackNote"
            @update:model-value="pickRange"
          />
        </section>

        <section class="cards">
          <div class="glass-card stat">
            <p class="label">{{ current ? `${appLabel(current)} · 总时长` : "全部应用 · 总时长" }}</p>
            <p class="value accent">{{ fmtDuration(report?.totalSeconds ?? 0) }}</p>
          </div>
          <div class="glass-card stat">
            <p class="label">平均每活跃日</p>
            <p class="value">{{ fmtDuration(report?.avgPerDay ?? 0) }}</p>
          </div>
          <div class="glass-card stat">
            <p class="label">活跃天数 / 应用数</p>
            <p class="value">
              {{ report?.activeDays ?? 0 }} <small>天</small> · {{ report?.appCount ?? 0 }} <small>个</small>
            </p>
          </div>
        </section>

        <template v-if="report">
          <section class="glass-card wide">
            <div class="cardhead">
              <h2>{{ current ? `${appLabel(current)} · 使用趋势` : "使用趋势" }}</h2>
              <span class="avg">平均 {{ fmtDuration(report.avgPerDay) }}/天</span>
            </div>
            <DayBars :labels="trend.labels" :series="trend.series" :label-interval="trend.interval" />
          </section>

          <section class="glass-card wide">
            <h2>{{ current ? `${appLabel(current)} · 各小时分布` : "24 小时分布" }}</h2>
            <HourlyChart :slices="report.hourly" :height="200" />
          </section>
        </template>
        <BallLoader v-else-if="loading" label="加载中…" />
        <p v-else-if="err" class="err">读取失败：{{ err }}</p>
        <p v-else class="empty2">这个范围没有记录</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.detail {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.viewseg {
  display: inline-flex;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  padding: 4px;
  background: var(--surface);
  width: fit-content;
}

.vbtn {
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 13px;
  font-family: inherit;
  padding: 6px 16px;
  border-radius: var(--r-sm);
  cursor: pointer;
}

.vbtn.active {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.rangecard {
  padding: 14px 16px;
}

.itemsgrid {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 16px;
  align-items: start;
}

.itemsdetail {
  min-width: 0;
}

.itemsday {
  font-size: 13px;
  font-weight: 700;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

@media (max-width: 980px) {
  .itemsgrid {
    grid-template-columns: 1fr;
  }
}

.itemsview {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 18px;
}

.itemshead {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.itemshead h2 {
  margin: 0;
}

.head {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.head h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
}

.sub {
  font-size: 12px;
  color: var(--text-muted);
}

/* 两列等高：底部对齐（窗口模式下不再一高一低） */
.split {
  display: grid;
  grid-template-columns: 268px 1fr;
  gap: 14px;
  align-items: stretch;
}

.right {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-width: 0;
}

.glass-card {
  padding: 14px 16px;
}

.glass-card.wide {
  padding-bottom: 12px;
}

.glass-card h2 {
  margin: 0 0 10px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}

.cardhead {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.avg {
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

.rangerow {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.rangehead {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.rangehead h2 {
  margin: 0;
}

.rangetext {
  font-size: 15px;
  font-weight: 700;
  color: var(--text);
}

.rangeopts {
  display: inline-flex;
  gap: 8px;
}

.rangeopt {
  border: 0;
  background: transparent;
  color: var(--text-faint);
  font-size: 13px;
  font-family: inherit;
  padding: 2px 8px;
  border-radius: var(--r-sm);
  cursor: pointer;
  transition: color var(--dur), background var(--dur);
}

.rangeopt:hover {
  color: var(--text);
  background: var(--surface-hover);
}

.rangeopt.active {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.spantext {
  margin: 0;
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

.note {
  margin: 2px 0 0;
  font-size: 11px;
  color: var(--accent-text);
}

.cards {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 14px;
  align-items: stretch;
}

.stat .label {
  margin: 0 0 6px;
  font-size: 12px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.stat .value {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.stat .value.accent {
  color: var(--accent-text);
}

.stat .value.num2 {
  font-size: 14px;
}

.stat small {
  font-size: 12px;
  font-weight: 400;
  color: var(--text-faint);
}

/* 应用列表撑满所在列，底部与右侧列对齐 */
/* 外层：撑满右列（不参与高度计算），内层绝对定位贴合 → 卡片底部与右列平齐 */
.applist-wrap {
  position: relative;
  min-height: 320px;
}

.applist {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.listhead {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.listhead h2 {
  margin: 0;
}

.listbody {
  flex: 1;
  min-height: 240px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding-right: 4px;
}

.approw {
  display: grid;
  /* 序号位/图标位/名称/时长：图标列 22px，避免 16px 图标溢出压字 */
  grid-template-columns: 22px 1fr auto;
  grid-template-rows: auto auto;
  align-items: center;
  gap: 4px 10px;
  border: 1px solid transparent;
  background: var(--surface);
  color: var(--text-muted);
  border-radius: var(--r-md);
  padding: 7px 9px;
  font-family: inherit;
  font-size: 12px;
  cursor: pointer;
  text-align: left;
  transition: background var(--dur), border-color var(--dur), transform var(--dur);
}

.approw:hover {
  background: var(--surface-hover);
  transform: translateX(calc(2px * var(--motion)));
}

.approw.active {
  border-color: var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent-text);
}

.adot {
  grid-column: 1;
  justify-self: center;
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.aicon {
  grid-column: 1;
  justify-self: center;
  width: 16px;
  height: 16px;
  border-radius: 4px;
  object-fit: contain;
}

.aname {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}

.approw.active .aname {
  color: var(--accent-text);
}

.atime {
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

.abar {
  grid-column: 1 / 4;
  display: block;
  height: 4px;
  border-radius: 2px;
  background: var(--chart-rail);
  overflow: hidden;
}

.abar i {
  display: block;
  height: 100%;
  border-radius: 2px;
  transition: width var(--dur) ease;
}

.empty {
  color: var(--text-faint);
  font-size: 12px;
  text-align: center;
  padding: 18px 0;
}

.err {
  margin: 0;
  font-size: 13px;
  color: var(--danger);
}

.empty2 {
  margin: 0;
  font-size: 13px;
  color: var(--text-faint);
}
</style>
