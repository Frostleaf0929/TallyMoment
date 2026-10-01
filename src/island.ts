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
  opacity: number;
  accentMode: string;
  hideDelaySec: number;
  idleWidth: number;
  hideMode: string;
  snapReveal: number;
  posMode: string;
  snapEnabled: boolean;
  clickThrough: boolean;
  alwaysTop: boolean;
  snapWake: string;
}

// 主程序强调色（与 src/styles/theme.css 一致）
const ACCENT_MAP: Record<string, string> = {
  indigo: "#7b84ec", teal: "#58b3c4", green: "#6fb59a",
  violet: "#a08fe0", amber: "#d3a35e", rose: "#d3859b",
};

function hexA(hex: string, a: number): string {
  const m = hex.replace("#", "");
  const v = m.length === 3 ? m.split("").map((c) => c + c).join("") : m;
  const n = parseInt(v, 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
}

function applyAccent(mode: string) {
  let hex = "#b8e34b"; // 终末地超充黄绿
  if (mode === "accent") {
    const key = localStorage.getItem("ui.accent") || "rose";
    hex = ACCENT_MAP[key] || localStorage.getItem("ui.customAccent") || "#7b84ec";
  }
  const root = document.documentElement;
  root.style.setProperty("--acc", hex);
  root.style.setProperty("--acc-line", hexA(hex, 0.45));
}

let state: "idle" | "normal" | "expanded" | "snap" = "normal";
let snapEdge: "top" | "bottom" | "left" | "right" | null = null;
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

function setState(s: "idle" | "normal" | "expanded" | "snap") {
  state = s;
  if (s !== "snap") snapEdge = null;
  document.body.classList.remove(
    "idle", "normal", "expanded", "snap",
    "snap-top", "snap-bottom", "snap-left", "snap-right"
  );
  document.body.classList.add(s);
  if (s === "snap" && snapEdge) document.body.classList.add(`snap-${snapEdge}`);
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
  const unit = (badge: string, name: string, num: string, acc = false) =>
    `<span class="unit"><span class="urow"><span class="badge">${badge}</span><span class="mod-name">${esc(name)}</span></span>` +
    `<span class="mod-num${acc ? " acc" : ""}">${num}</span></span>`;
  let first = true;
  for (const m of modules.filter((x) => ["focus", "next", "done", "clock"].includes(x))) {
    if (!first) parts.push(`<span class="vsep"></span>`);
    first = false;
    if (m === "focus") {
      parts.push(
        data.focusName
          ? unit(data.paused ? "PAUSED" : "FOCUS", data.focusName, fmtSec(data.focusSec), !data.paused)
          : unit("IDLE", "未在专注", "--:--")
      );
    } else if (m === "next") {
      parts.push(
        data.nextTask
          ? unit("NEXT", data.nextTask.content, fmtDue(data.nextTask.dueTs))
          : unit("NEXT", "暂无任务", "—")
      );
    } else if (m === "done") {
      parts.push(unit("DONE", "今日完成", `${data.doneToday}<small>项</small>`, true));
    } else if (m === "clock") {
      const now = new Date();
      parts.push(unit("TIME", "时钟", `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`));
    }
  }
  main.innerHTML = parts.join("");
}

function applyData() {
  if (!data) return;
  idleEnabled = data.idleEnabled;
  document.body.classList.toggle("paused", data.paused);
  document.body.classList.toggle("focusing", !!data.focusName && !data.paused);
  // 透明度（40~100%）→ 胶囊底色 alpha
  const rootStyle0 = document.documentElement.style;
  rootStyle0.setProperty("--pill-a", (data.opacity / 100).toFixed(2));
  // idle/snap 胶囊的对齐与尺寸（与后端光标命中矩形同式）
  document.body.classList.toggle("pos-left", data.posMode === "left");
  document.body.classList.toggle("pos-right", data.posMode === "right");
  const rootStyle = document.documentElement.style;
  rootStyle.setProperty("--idle-w", `${Math.round(data.idleWidth)}px`);
  rootStyle.setProperty("--reveal", `${Math.round(data.snapReveal)}px`);
  // 关闭"自动隐藏"时若正缩着，立刻回正常态
  if (!idleEnabled && (state === "idle" || state === "snap")) {
    setState("normal");
    return;
  }
  applyAccent(data.accentMode);

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
  if (!dragStarted) {
    // 吸附态点本体 = 唤回正常胶囊；其余 = 展开/收起
    if (state === "snap") setState("normal");
    else setState(state === "expanded" ? "normal" : "expanded");
  }
});

// WinIsland 式悬停：进入放大、离开延迟缩小（展开态不缩回，点本体再收起）。
// 事件挂在胶囊本体（bar）上而不是 document：方案 A 里窗口比胶囊大，
// 鼠标离开胶囊但仍在窗口内时也算"离开"；穿透态下事件由后端光标轮询兜底恢复。
let hideTimer: number | undefined;
bar.addEventListener("mouseenter", () => {
  if (hideTimer !== undefined) {
    clearTimeout(hideTimer);
    hideTimer = undefined;
  }
  // 吸附 + "点击弹出"模式：悬停不唤回，点一下才弹
  if (state === "snap" && data?.snapWake === "click") return;
  if (state === "idle" || state === "snap") setState("normal");
});
bar.addEventListener("mouseleave", () => {
  if (state !== "normal" || !idleEnabled) return;
  const delay = Math.max(0, data?.hideDelaySec ?? 1) * 1000;
  if (delay === 0) {
    setState("idle");
    return;
  }
  hideTimer = window.setTimeout(() => {
    hideTimer = undefined;
    if (state !== "normal") return;
    // 靠边吸附开着就尝试吸附（后端判定贴哪条边/够不够近，不够近回退缩小）
    setState(data?.snapEnabled ? "snap" : "idle");
  }, delay);
});

// 后端吸附判定结果：回填方向类（窗口已由后端贴边就位）；不靠边则回退缩小
listen<string>("island-snap", (ev) => {
  if (state !== "snap") return; // 用户已提前唤回
  const edge = ev.payload;
  if (edge === "none") {
    setState("idle");
    return;
  }
  if (edge === "top" || edge === "bottom" || edge === "left" || edge === "right") {
    snapEdge = edge;
    document.body.classList.add(`snap-${edge}`);
  }
}).catch(() => {});

// 吸附态"靠近自动弹出"：光标靠近露出条，由后端轮询唤醒（区域裁剪后窗口很小，DOM 事件不可靠）
listen("island-wake", () => {
  if (state === "snap" && data?.snapWake !== "click") setState("normal");
}).catch(() => {});

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
// 窗口显示后再揭掉 boot：入场滑入动画由 CSS 过渡播出（方案 A 里不再有窗口位移动画）
invoke("island_ready")
  .catch(() => {})
  .then(() => {
    requestAnimationFrame(() => document.body.classList.remove("boot"));
  });
