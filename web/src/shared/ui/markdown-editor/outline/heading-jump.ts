import { StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, type DecorationSet } from "@codemirror/view";

/** 跳转后标题行的高亮时长（毫秒），与样式中的淡出动画保持一致。 */
const FLASH_MS = 900;

/** 平滑滚动的等待上限（毫秒），超时后直接做精确校正。 */
const SMOOTH_SETTLE_MS = 420;

/** 标题距视口顶部的留白（像素）。 */
const TOP_MARGIN_PX = 12;

/** 设置或清除跳转高亮的效果；null 表示清除。 */
const flashEffect = StateEffect.define<number | null>();

/**
 * 经大纲跳转到某个标题的效果，值为标题起点。
 *
 * 文档末尾的标题无法滚到视口顶部，按滚动位置推算会高亮成别的标题；
 * 大纲跟踪收到该效果后直接锁定目标标题，直到用户再次手动滚动。
 */
export const headingJumpEffect = StateEffect.define<number>();

/** 跳转高亮的行装饰。 */
const flashLine = Decoration.line({ class: "cm-md-flash" });

/**
 * 跳转高亮状态：一次只高亮一行，随文档变更映射位置。
 */
export const headingFlashField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(value, transaction) {
    let next = value.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (!effect.is(flashEffect)) continue;
      next = effect.value === null ? Decoration.none : Decoration.set([flashLine.range(effect.value)]);
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});

/**
 * 平滑滚动到标题，并短暂高亮该行帮助定位。
 *
 * 视口外的行高是估算值，因此先按估算位置平滑滚动，
 * 滚动停下后再用 CodeMirror 自带的精确定位校正残差。
 *
 * @param view 编辑器视图
 * @param from 标题行起始偏移
 * @returns 无
 */
export function jumpToHeading(view: EditorView, from: number): void {
  const position = Math.min(from, view.state.doc.length);
  const line = view.state.doc.lineAt(position);
  const scroller = view.scrollDOM;
  // 1. 光标放到标题末尾并高亮，焦点交还编辑器便于继续输入
  view.dispatch({ selection: { anchor: line.to }, effects: [flashEffect.of(line.from), headingJumpEffect.of(from)] });
  view.focus();
  // 2. 按估算位置平滑滚动
  const documentOffset = view.documentTop - scroller.getBoundingClientRect().top + scroller.scrollTop;
  const target = view.lineBlockAt(line.from).top + documentOffset - TOP_MARGIN_PX;
  scroller.scrollTo({ top: Math.max(0, target), behavior: prefersReducedMotion() ? "auto" : "smooth" });
  // 3. 停下后精确校正，再定时清除高亮
  const settle = () => {
    scroller.removeEventListener("scrollend", settle);
    clearTimeout(fallback);
    if (!view.dom.isConnected) return;
    view.dispatch({ effects: EditorView.scrollIntoView(line.from, { y: "start", yMargin: TOP_MARGIN_PX }) });
  };
  const fallback = setTimeout(settle, SMOOTH_SETTLE_MS);
  scroller.addEventListener("scrollend", settle, { once: true });
  setTimeout(() => {
    if (view.dom.isConnected && view.state.field(headingFlashField, false)?.size) view.dispatch({ effects: flashEffect.of(null) });
  }, FLASH_MS);
}

/**
 * 判断用户是否偏好减少动效。
 *
 * @returns 偏好减少动效时为 true
 */
function prefersReducedMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}
