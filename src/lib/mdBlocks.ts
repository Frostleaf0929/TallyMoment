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
