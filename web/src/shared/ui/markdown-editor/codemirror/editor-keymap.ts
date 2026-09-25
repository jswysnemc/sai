import { Prec, type Extension } from "@codemirror/state";
import { keymap, type Command } from "@codemirror/view";
import { insertMathBlock, insertCodeBlock, setHeading, toggleList, toggleQuote } from "./commands/block-commands";
import { BOLD, INLINE_CODE, insertLink, ITALIC, STRIKE, toggleInline } from "./commands/inline-commands";
import { smartBackspace, smartEnter, smartTab } from "./commands/structure-keys";
import { insertTable } from "./wysiwyg-table-actions";

/** 快捷键依赖的外部回调，通过 getter 读取最新值，避免回调变化时重建扩展。 */
export type EditorKeymapOptions = {
  /** Ctrl+/ 切换源码与预览；缺省时该快捷键不生效 */
  onToggleMode?: () => void;
  /** 双语文案取值函数，用于插入表格时的表头文字 */
  t: (en: string, zh: string) => string;
};

/**
 * 两种模式共用的快捷键：格式与模式切换。
 *
 * 快捷键对齐 Typora，并为浏览器保留的 Ctrl+数字、Ctrl+T 提供 Ctrl+Alt 备用组合。
 *
 * @param options 获取外部回调的函数
 * @returns CodeMirror 扩展
 */
export function sharedKeymap(options: () => EditorKeymapOptions): Extension {
  const headings: { key: string; run: Command }[] = [];
  for (let level = 0; level <= 6; level += 1) {
    headings.push({ key: `Mod-${level}`, run: setHeading(level) });
    headings.push({ key: `Mod-Alt-${level}`, run: setHeading(level) });
  }
  return Prec.high(
    keymap.of([
      ...headings,
      { key: "Mod-b", run: toggleInline(BOLD) },
      { key: "Mod-i", run: toggleInline(ITALIC) },
      { key: "Mod-e", run: toggleInline(INLINE_CODE) },
      { key: "Mod-Shift-`", run: toggleInline(INLINE_CODE) },
      { key: "Mod-Shift-x", run: toggleInline(STRIKE) },
      { key: "Mod-k", run: insertLink() },
      { key: "Mod-Shift-q", run: toggleQuote() },
      { key: "Mod-Shift-[", run: toggleList("ordered") },
      { key: "Mod-Shift-]", run: toggleList("bullet") },
      { key: "Mod-Shift-k", run: insertCodeBlock() },
      { key: "Mod-Shift-m", run: insertMathBlock() },
      {
        key: "Mod-Alt-t",
        run: (view) => insertTable(view, 3, 2, (index) => options().t(`Column ${index + 1}`, `列 ${index + 1}`)),
      },
      {
        key: "Mod-/",
        run: () => {
          const toggle = options().onToggleMode;
          toggle?.();
          return Boolean(toggle);
        },
      },
    ])
  );
}

/**
 * 预览（所见即所得）模式专用的结构键：Enter、Backspace、Tab。
 *
 * 优先级高于 Markdown 自带的列表续写，未命中时返回 false 交还给它。
 *
 * @returns CodeMirror 扩展
 */
export function liveKeymap(): Extension {
  return Prec.highest(
    keymap.of([
      { key: "Enter", run: smartEnter() },
      { key: "Backspace", run: smartBackspace() },
      { key: "Tab", run: smartTab(false) },
      { key: "Shift-Tab", run: smartTab(true) },
    ])
  );
}
