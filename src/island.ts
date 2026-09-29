/**
 * 原子岛独立入口（vanilla TS，无框架）：island.html 专用。
 * 终末地充电 HUD 风 + WinIsland 式三态：
 * idle（无悬停缩小）→ normal（悬停完整胶囊）→ expanded（点击本体展开）。
 * 点击 vs 拖动由前端判别：按下后位移超过阈值才交给系统拖动，原地松开视为点击。
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface NextTask { id: number; content: string; dueTs: number }
interface IslandData {
  focusName: string | null;
  focusSec: number;
  doneToday: number;
  paused: boolean;
  nextTask: NextTask | null;
  tasks: NextTask[];
  idleEnabled: boolean;
}

let state: "idle" | "normal" | "expanded" = "normal";
let idleEnabled = true;
let modules: string[] = ["focus", "next", "done"];
let data: IslandData | null = null;

const $ = (id: string) => document.getElementById(id)!;
const bar = $("bar");
const main = $("main");

function fmtSec(s: number): string {
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  if (m >= 60) {
    const h = Math.floor(m / 60);
    return `${h}:${String(m % 60).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
  }
  return `${m}:${String(sec).padStart(2, "0")}`;
}
function fmtDue(ts: number): string {
  const d = new Date(ts * 1000);
  const now = new Date();
  const hm = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  return d.toDateString() === now.toDateString() ? hm : `${d.getMonth() + 1}/${d.getDate()} ${hm}`;
}
function esc(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function setState(s: "idle" | "normal" | "expanded") {
  state = s;
  document.body.className = s;
  applyData();
  invoke("island_set_state", { state: s }).catch(() => {});
}

// 电标圆环：专注时每分钟转一圈，保持生命感
function ringProgress(sec: number): number {
  return (sec % 60) / 60;
}

function renderModules() {
  if (!data) return;
  const parts: string[] = [];
  for (const m of modules.filter((x) => ["focus", "next", "done", "clock"].includes(x))) {
    if (m === "focus") {
      parts.push(
        data.focusName
          ? `<div class="badge-row"><span class="badge">${data.paused ? "PAUSED" : "FOCUS"}</span><span class="mod-name">${esc(data.focusName)}</span></div>` +
            `<span class="mod-num${data.paused ? "" : " acc"}">${fmtSec(data.focusSec)}</span>`
          : `<div class="badge-row"><span class="badge">IDLE</span><span class="mod-name">未在专注</span></div>` +
            `<span class="mod-num" style="color:var(--ink-dim)">--:--</span>`
      );
    } else if (m === "next") {
      parts.push(
        data.nextTask
          ? `<div class="badge-row"><span class="badge">NEXT</span><span class="mod-name">${esc(data.nextTask.content)}</span></div>` +
            `<span class="mod-num">${fmtDue(data.nextTask.dueTs)}</span>`
          : `<div class="badge-row"><span class="badge">NEXT</span><span class="mod-name">暂无任务</span></div>` +
            `<span class="mod-num" style="color:var(--ink-dim)">—</span>`
      );
    } else if (m === "done") {
      parts.push(
        `<div class="badge-row"><span class="badge">DONE</span><span class="mod-name">今日完成</span></div>` +
        `<span class="mod-num acc">${data.doneToday}<small>项</small></span>`
      );
    } else if (m === "clock") {
      const now = new Date();
      parts.push(
        `<div class="badge-row"><span class="badge">TIME</span><span class="mod-name">时钟</span></div>` +
        `<span class="mod-num">${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}</span>`
      );
    }
  }
  main.innerHTML = parts.join("");
}

function applyData() {
  if (!data) return;
  idleEnabled = data.idleEnabled;
  document.body.classList.toggle("paused", data.paused);
  document.body.classList.toggle("focusing", !!data.focusName && !data.paused);

  const ring = document.querySelector<SVGCircleElement>(".ring-fg");
  if (ring) {
    const p = data.focusName && !data.paused ? ringProgress(data.focusSec) : 0;
    ring.style.strokeDashoffset = String(94.2 * (1 - p));
  }

  $("done-n").textContent = String(data.doneToday);
  $("idle-done").innerHTML = `✓<b>${data.doneToday}</b>`;

  // idle 核心：专注时长 > 下个任务倒计时 > 今日完成
  const core = $("idle-core");
  if (data.focusName) {
    core.innerHTML = `<span class="l">${data.paused ? "暂停" : "专注"}</span>${fmtSec(data.focusSec)}`;
  } else if (data.nextTask) {
    const left = data.nextTask.dueTs - Math.floor(Date.now() / 1000);
    core.innerHTML = `<span class="l">下一项</span>${left > 0 ? fmtSec(left) + " 后" : "已到期"} · ${esc(data.nextTask.content)}`;
  } else {
    core.innerHTML = `<span class="l">今日</span>完成 ${data.doneToday} 项`;
  }

  renderModules();

  $("next-name").innerHTML = data.nextTask
    ? `${esc(data.nextTask.content)}<span class="t">${fmtDue(data.nextTask.dueTs)}</span>`
    : `<span style="color:var(--ink-dim)">暂无待办，去〈待办〉添加</span>`;
  const tl = $("tasks");
  tl.innerHTML = data.tasks.length
    ? data.tasks
        .map((t) => {
          const over = t.dueTs < Date.now() / 1000;
          return `<div class="task-row" data-id="${t.id}"><span class="sq"></span><span class="nm">${esc(t.content)}</span><span class="due${over ? " over" : ""}">${fmtDue(t.dueTs)}</span></div>`;
        })
        .join("")
    : `<div class="empty">没有带截止时间的待办</div>`;
}

async function refresh() {
  try {
    data = await invoke<IslandData>("island_data");
    applyData();
  } catch {
    /* 后端未就绪时静默 */
  }
}

function tickClock() {
  const now = new Date();
  $("clock").textContent = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
}

// ===== 点击 vs 拖动判别 =====
let downX = 0, downY = 0, dragStarted = false, pressing = false;

bar.addEventListener("mousedown", (e) => {
  if (e.button !== 0) return;
  pressing = true;
  dragStarted = false;
  downX = e.screenX;
  downY = e.screenY;
});
window.addEventListener("mousemove", (e) => {
  if (!pressing || dragStarted) return;
  const dx = e.screenX - downX, dy = e.screenY - downY;
  if (dx * dx + dy * dy > 36) {
    dragStarted = true;
    invoke("island_start_drag").catch(() => {});
  }
});
window.addEventListener("mouseup", () => {
  if (!pressing) return;
  pressing = false;
  if (!dragStarted) setState(state === "expanded" ? "normal" : "expanded");
});

// WinIsland 式悬停：进入放大、离开缩小（展开态不缩回，点本体再收起）
document.addEventListener("mouseenter", () => {
  if (state === "idle") setState("normal");
});
document.addEventListener("mouseleave", () => {
  if (state === "normal" && idleEnabled) setState("idle");
});

// 任务完成 + 设置
$("tasks").addEventListener("click", (e) => {
  const row = (e.target as HTMLElement).closest(".task-row") as HTMLElement | null;
  if (row?.dataset.id) invoke("task_set_done", { id: Number(row.dataset.id) }).then(refresh).catch(() => {});
});
$("setbtn").addEventListener("click", () => invoke("island_open_settings").catch(() => {}));

// 模块配置热更新
listen<string[]>("island-modules", (ev) => {
  modules = Array.isArray(ev.payload) ? ev.payload : modules;
  renderModules();
}).catch(() => {});

invoke<string[]>("island_get_modules")
  .then((m) => {
    if (Array.isArray(m) && m.length) modules = m;
    renderModules();
  })
  .catch(() => {});

tickClock();
setInterval(tickClock, 1000);
refresh();
setInterval(refresh, 1000);
invoke("island_ready").catch(() => {});
