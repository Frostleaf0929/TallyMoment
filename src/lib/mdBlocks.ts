/**
 * 极简 Markdown 块解析 / 渲染（日志编辑器用）
 * 只支持我们工具栏会产出的语法：# 标题、- 列表、- [ ] 待办、> 引用、--- 分割线、
 * **加粗**、`代码`、![](图片)。这样既够"块式"排版，导出又仍是标准 Markdown。
 */

export type BlockKind = "heading" | "todo" | "list" | "quote" | "hr" | "text";

export interface Block {
  kind: BlockKind;
  /** 原始多行文本（原样保留，便于回写） */
  raw: string;
  /** 行数组 */
  lines: string[];
}

/** 把 Markdown 切成块：空行分隔；连续的列表/待办行合并为一块 */
export function parseBlocks(md: string): Block[] {
  const lines = md.split(String.fromCharCode(10));
  const blocks: Block[] = [];
  let buf: string[] = [];
  let kind: BlockKind = "text";

  const kindOf = (ln: string): BlockKind => {
    const t = ln.trim();
    if (/^#{1,6}\s/.test(t)) return "heading";
    if (/^-\s*\[[ xX]\]/.test(t)) return "todo";
    if (/^[-*]\s/.test(t)) return "list";
    if (t.startsWith(">")) return "quote";
    if (/^(-{3,}|\*{3,})$/.test(t)) return "hr";
    return "text";
  };

  const flush = () => {
    if (!buf.length) return;
    const raw = buf.join(String.fromCharCode(10));
    if (raw.trim() !== "") blocks.push({ kind, raw, lines: [...buf] });
    buf = [];
    kind = "text";
  };

  for (const ln of lines) {
    if (ln.trim() === "") {
      flush();
      continue;
    }
    const k = kindOf(ln);
    if (buf.length && k !== kind) flush();
    // 引用与分隔线各自成块；列表/待办同类合并
    if (!buf.length) kind = k;
    buf.push(ln);
  }
  flush();
  return blocks;
}

/** 最小内联渲染：**粗体**、`代码`（先转义再替换，避免 XSS） */
export function inline(text: string): string {
  const esc = text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
  return esc
    .replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
    .replace(/`([^`]+)`/g, "<code>$1</code>");
}

/** 把一行里的图片语法替换成可显示的 <img>（images 提供 name -> dataURL） */
export function renderLine(line: string, images: Record<string, string>): string {
  let out = inline(line);
  // ![](images/xxx.png) 或 <img src="images/xxx.png" width="50%">
  out = out.replace(
    /!\[\]\(([^)]+)\)/g,
    (_m, rel: string) => {
      const name = String(rel).split("/").pop() ?? "";
      const src = images[name];
      if (!src) return `<span class="miss">[缺图 ${name}]</span>`;
      return `<img class="mdimg" src="${src}" alt="" />`;
    }
  );
  out = out.replace(
    /&lt;img src=&quot;([^&]+)&quot;(?: width=&quot;([^&]+)&quot;)?\s*\/?&gt;/g,
    (_m, rel: string, w: string) => {
      const name = String(rel).split("/").pop() ?? "";
      const src = images[name];
      if (!src) return `<span class="miss">[缺图 ${name}]</span>`;
      return `<img class="mdimg" style="width:${w || "100%"}" src="${src}" alt="" />`;
    }
  );
  return out;
}

/** 块上下移动（返回新的 Markdown） */
export function moveBlock(md: string, index: number, dir: -1 | 1): string {
  const blocks = parseBlocks(md);
  const j = index + dir;
  if (index < 0 || j < 0 || index >= blocks.length || j >= blocks.length) return md;
  const tmp = blocks[index];
  blocks[index] = blocks[j];
  blocks[j] = tmp;
  return blocks.map((b) => b.raw).join(String.fromCharCode(10, 10));
}

/** 删除某块 */
export function removeBlock(md: string, index: number): string {
  const blocks = parseBlocks(md);
  blocks.splice(index, 1);
  return blocks.map((b) => b.raw).join(String.fromCharCode(10, 10));
}

/** 在末尾追加一块 */
export function appendBlock(md: string, raw: string): string {
  const sep = md.trim() ? String.fromCharCode(10, 10) : "";
  return md + sep + raw;
}

/* ---------- 选区格式化（Notion 式：对选中文字 / 光标所在行直接生效） ---------- */

const NL = String.fromCharCode(10);

export interface SelEdit {
  text: string;
  selStart: number;
  selEnd: number;
}

/** 选区覆盖到的行块（从行首到最后一行的行尾） */
function lineBlock(text: string, selStart: number, selEnd: number): [number, number] {
  const ls = selStart <= 0 ? 0 : text.lastIndexOf(NL, selStart - 1) + 1;
  const nl = text.indexOf(NL, selEnd);
  const le = nl === -1 ? text.length : nl;
  return [ls, le];
}

const RE_HEADING = /^(\s*)(#{1,6})(\s+)(.*)$/;
const RE_TODO = /^(\s*)-\s*\[[ xX]\]\s*/;
const RE_LIST = /^(\s*)[-*]\s+/;
const RE_QUOTE = /^(\s*)>\s?/;
const RE_NUM = /^(\s*)\d+\.\s+/;

/** 剥掉行首的块前缀（标题/待办/列表/引用/序号），返回缩进与剩余内容 */
function stripPrefix(line: string): { indent: string; rest: string } {
  const m =
    line.match(RE_HEADING) ||
    line.match(RE_TODO) ||
    line.match(RE_LIST) ||
    line.match(RE_QUOTE) ||
    line.match(RE_NUM);
  if (!m) return { indent: "", rest: line };
  return { indent: m[1] ?? "", rest: line.slice(m[0].length) };
}

/** 行前缀转换：heading=级别循环（无→#→##→###→无），todo/list/quote=开关，plain=转回普通段落 */
export function toggleLinePrefix(
  text: string,
  selStart: number,
  selEnd: number,
  kind: "heading" | "todo" | "list" | "quote" | "plain"
): SelEdit {
  const [ls, le] = lineBlock(text, selStart, selEnd);
  const out = text
    .slice(ls, le)
    .split(NL)
    .map((line) => {
      if (line.trim() === "") return line;
      switch (kind) {
        case "heading": {
          const m = line.match(RE_HEADING);
          if (m) {
            const lvl = m[2].length;
            return lvl >= 3
              ? m[1] + m[4]
              : m[1] + "#".repeat(lvl + 1) + " " + m[4];
          }
          const p = stripPrefix(line);
          return p.indent + "# " + p.rest;
        }
        case "todo": {
          if (RE_TODO.test(line)) return line.replace(RE_TODO, "$1");
          const p = stripPrefix(line);
          return p.indent + "- [ ] " + p.rest;
        }
        case "list": {
          if (RE_TODO.test(line)) return line.replace(RE_TODO, "$1- ");
          if (RE_LIST.test(line)) return line.replace(RE_LIST, "$1");
          const p = stripPrefix(line);
          return p.indent + "- " + p.rest;
        }
        case "quote": {
          if (RE_QUOTE.test(line)) return line.replace(RE_QUOTE, "$1");
          const p = stripPrefix(line);
          return p.indent + "> " + p.rest;
        }
        case "plain": {
          const m =
            line.match(RE_HEADING) ||
            line.match(RE_TODO) ||
            line.match(RE_LIST) ||
            line.match(RE_QUOTE) ||
            line.match(RE_NUM);
          return m ? m[1] + line.slice(m[0].length) : line;
        }
      }
    })
    .join(NL);
  return { text: text.slice(0, ls) + out + text.slice(le), selStart: ls, selEnd: ls + out.length };
}

/** 内联包裹：**加粗** / *斜体* / <u></u> / ~~删除线~~ / `代码`；同一范围再点一次 = 取消 */
export function wrapSelection(
  text: string,
  selStart: number,
  selEnd: number,
  open: string,
  close: string
): SelEdit {
  const sel = text.slice(selStart, selEnd);
  const before = text.slice(0, selStart);
  const after = text.slice(selEnd);
  if (sel.length >= open.length + close.length && sel.startsWith(open) && sel.endsWith(close)) {
    const inner = sel.slice(open.length, sel.length - close.length);
    return { text: before + inner + after, selStart, selEnd: selStart + inner.length };
  }
  if (before.endsWith(open) && after.startsWith(close)) {
    const s = selStart - open.length;
    const e = selEnd + close.length;
    const inner = text.slice(s + open.length, e - close.length);
    return { text: text.slice(0, s) + inner + text.slice(e), selStart: s, selEnd: s + inner.length };
  }
  if (selStart === selEnd) {
    return {
      text: before + open + close + after,
      selStart: selStart + open.length,
      selEnd: selStart + open.length,
    };
  }
  return { text: before + open + sel + close + after, selStart, selEnd: selEnd + open.length };
}

/** 光标处插入独立一行（分割线等），前后自动补空行 */
export function insertAtCursor(
  text: string,
  selStart: number,
  selEnd: number,
  snippet: string
): SelEdit {
  const before = text.slice(0, selStart);
  const after = text.slice(selEnd);
  const lead = !before ? "" : before.endsWith(NL + NL) ? "" : before.endsWith(NL) ? NL : NL + NL;
  const tail = after.startsWith(NL) || after === "" ? "" : NL;
  const ins = lead + snippet + tail;
  return { text: before + ins + after, selStart: selStart + ins.length, selEnd: selStart + ins.length };
}
