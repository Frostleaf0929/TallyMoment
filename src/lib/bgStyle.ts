/**
 * 背景图渲染口径（唯一定义处）——编辑器的"虚拟屏幕"与真实全屏提醒**共用**此函数，
 * 杜绝"两套模板各写一份、改一处漏一处"导致预览与实机不一致。
 *
 * 参数语义（与后端 ui.reminder_bg_imgcfg 一致）：
 * - fit: "cover"（铺满裁剪）/ "contain"（完整显示）
 * - align: "X% Y%"（object-position，控制裁剪偏移）
 * - zoom: 50~300（百分比，transform: scale 围绕容器中心放大/缩小）
 */
export interface BgCfgLike {
  fit: string;
  align: string;
  zoom: number;
}

export function bgImgStyle(cfg: BgCfgLike): Record<string, string> {
  const fit = cfg.fit === "contain" ? "contain" : "cover";
  const align = cfg.align && cfg.align.includes("%") ? cfg.align : "50% 50%";
  const zoom = Number.isFinite(cfg.zoom) ? Math.min(300, Math.max(50, cfg.zoom)) : 100;
  return {
    objectFit: fit,
    objectPosition: align,
    // 显式 transform-origin：缩放围绕容器中心（两边同口径）
    transformOrigin: "center center",
    transform: `scale(${(zoom / 100).toFixed(3)})`,
  };
}

/** 蒙版层不透明度（scrim 0~100 → 0~1） */
export function bgScrimOpacity(scrim: number): string {
  const v = Number.isFinite(scrim) ? Math.min(100, Math.max(0, scrim)) : 100;
  return (v / 100).toFixed(2);
}
