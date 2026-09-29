/**
 * 原子岛独立入口（vanilla TS，无框架）：island.html 专用。
 * 不加载 Vue / naive-ui / echarts——窗口毫秒级可用（此前走主前端要 2 秒+）。
 * 数据每秒向后端拉取；模块配置由个性化页下发（island-modules 事件）。
 */
import { invoke } from "@tauri-apps/api/core";

interface NextTask {
  id: number;
  content: string;
  dueTs: number;
}
interface IslandData {
  focusName: string | null;
  focusSec: number;
  doneToday: number;
  paused: boolean;
  nextTask: NextTask | null;
  tasks: NextTask[];
}

const MODULE_FALLBACK = ["focus", "next", "done"];

const barMods = document.getElementById("bar-mods") as HTMLDivElement;
const tasksBox = document.getElementById("tasks") as HTMLDivElement;
const tasksWrap = document.getElementById("tasks-wrap") as HTMLDivElement;
const tgBtn = document.getElementById("tg") as HTMLButtonElement;

let data: IslandData | null = null;
let modules: string[] = [...MODULE_FALLBACK];
let expanded = false;
let now = Math.floor(Date.now() / 1000);

function pad(n: number): string {
  return `${n}`.padStart(2, "0");
}

function fmtDelta(sec: number): string {
  const s = Math.max(0, sec);
  if (s >= 3600) return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
  if (s >= 60) return `${Math.floor(s / 60)}m ${s % 60}s`;
  return `${s}s`;
}

function esc(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
}

/** 单个模块的 HTML（每秒随数据重建该段） */
function modHtml(m: string): string {
  const d = data;
  switch (m) {
    case "focus": {
      const t = !d
        ? "…"
        : d.paused
        ? "已暂停"
        : d.focusName
        ? `${d.focusName} · ${fmtDelta(d.focusSec)}`
        : "休息中";
      return `<span class="mod focus${d?.paused ? " off" : ""}" title="${esc(t)}">${esc(t)}</span>`;
    }
    case "next": {
      const t = d?.nextTask;
      const body = t
        ? `${esc(t.content)} · ${fmtDelta(t.dueTs - now)}`
        : "没有排期中的任务";
      const title = t ? esc(t.content) : "";
      return `<span class="mod next" title="${title}"><span class="tag">下个</span>${body}</span>`;
    }
    case "done":
      return `<span class="mod done"><b class="num">${d?.doneToday ?? 0}</b> 今日</span>`;
    case "clock": {
      const dt = new Date(now * 1000);
      return `<span class="mod clock num">${pad(dt.getHours())}:${pad(dt.getMinutes())}</span>`;
    }
    default:
      return "";
  }
}

function renderMods(): void {
  barMods.innerHTML = modules
    .map((m, i) => modHtml(m) + (i < modules.length - 1 ? `<span class="div"></span>` : ""))
    .join("");
}

function renderTasks(): void {
  if (!expanded) return;
  const list = data?.tasks ?? [];
  const rows = list
    .map(
      (t) =>
        `<div class="trow"><button class="ring" data-id="${t.id}" title="标记完成"></button>` +
        `<span class="tname" title="${esc(t.content)}">${esc(t.content)}</span>` +
        `<span class="tdue num">${fmtDelta(t.dueTs - now)}</span></div>`
    )
    .join("");
  tasksBox.innerHTML =
    `<p class="thead">待办 · 按截止排序</p>` +
    rows +
    (list.length ? "" : `<p class="tempty">没有排期中的任务</p>`);
}

async function refresh(): Promise<void> {
  try {
    data = await invoke<IslandData>("island_data");
    renderMods();
    renderTasks();
  } catch {
    /* 后端忙就等下一秒 */
  }
}

async function toggleExpand(): Promise<void> {
  expanded = !expanded;
  tasksWrap.style.display = expanded ? "block" : "none";
  tgBtn.classList.toggle("flip", expanded);
  renderTasks();
  await invoke("island_set_expanded", { expanded }).catch(() => {});
}

tgBtn.addEventListener("click", () => void toggleExpand());

tasksBox.addEventListener("click", (e) => {
  const el = (e.target as HTMLElement).closest(".ring") as HTMLElement | null;
  if (!el) return;
  const id = Number(el.dataset.id);
  if (id) {
    void invoke("task_set_done", { id, done: true })
      .catch(() => {})
      .then(refresh);
  }
});

document.getElementById("setbtn")!.addEventListener("click", () => {
  invoke("island_open_settings").catch(() => {});
});

setInterval(() => {
  now = Math.floor(Date.now() / 1000);
  void refresh();
}, 1000);

void refresh();

void invoke<string[]>("island_get_modules")
  .then((m) => {
    if (m.length) {
      modules = m;
      renderMods();
    }
  })
  .catch(() => {});

// 个性化里改了模块配置 → 实时刷新
import("@tauri-apps/api/event")
  .then(({ listen }) =>
    listen<string[]>("island-modules", (e) => {
      modules = e.payload;
      renderMods();
    })
  )
  .catch(() => {});

// 前端就绪：请 Rust 侧显示窗口
invoke("island_ready").catch(() => {});
