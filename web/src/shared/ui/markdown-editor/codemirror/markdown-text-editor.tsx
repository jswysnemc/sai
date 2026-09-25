import { Compartment, EditorState, Transaction } from "@codemirror/state";
import { EditorView, type ViewUpdate } from "@codemirror/view";
import { useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState, type MouseEvent, type Ref } from "react";
import { useI18n } from "../../../../features/i18n/use-i18n";
import { jumpToHeading } from "../outline/heading-jump";
import type { OutlineHeading } from "../outline/outline-model";
import { baseExtensions, presentationExtensions } from "./editor-extensions";
import type { ImageUrlResolver } from "./editor-facets";
import { blockFormatActions, inlineFormatActions, quoteAction } from "./editor-format-actions";
import { recordEmittedValue, resolveExternalValue } from "./external-value";
import { buildContextEntries, type MenuEntry } from "./menus/context-menu-entries";
import { MarkdownContextMenu } from "./menus/markdown-context-menu";
import { SelectionBubble } from "./menus/selection-bubble";
import { useSelectionBubble } from "./menus/use-selection-bubble";
import { cellRefOf } from "./wysiwyg-table-actions";
import "./markdown-text-editor.css";
import "./wysiwyg-blocks.css";

/** 对外暴露的命令句柄。 */
export type MarkdownTextEditorHandle = {
  /** 平滑滚动到指定位置所在的行并高亮 */
  jumpTo: (from: number) => void;
};

type MarkdownTextEditorProps = {
  ref?: Ref<MarkdownTextEditorHandle>;
  value: string;
  onChange: (value: string) => void;
  /** true 为所见即所得预览，false 为源码 */
  live: boolean;
  dark: boolean;
  readOnly?: boolean;
  /** 是否自动换行，缺省为换行 */
  wrap?: boolean;
  /** 相对图片地址解析函数 */
  resolveImageUrl?: ImageUrlResolver;
  /** Ctrl+/ 切换模式 */
  onToggleMode?: () => void;
  /** 标题列表变化回调 */
  onOutline?: (headings: OutlineHeading[]) => void;
  /** 视口所在标题变化回调 */
  onActiveHeading?: (index: number) => void;
};

/** 右键菜单状态。 */
type MenuState = { x: number; y: number; entries: MenuEntry[]; inTable: boolean };

/**
 * 基于 CodeMirror 6 的 Markdown 文本编辑器。
 *
 * 文档本身始终是 Markdown 源码，预览靠装饰层隐藏语法标记实现，
 * 因此两种模式之间往返不会改写用户的原始写法——这是与富文本方案的关键差别。
 *
 * @param props 文档内容、变更回调、模式、主题深浅、只读状态、换行与大纲回调
 * @returns 编辑器容器
 */
export function MarkdownTextEditor({
  ref,
  value,
  onChange,
  live,
  dark,
  readOnly = false,
  wrap = true,
  resolveImageUrl,
  onToggleMode,
  onOutline,
  onActiveHeading,
}: MarkdownTextEditorProps) {
  const { t } = useI18n();
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const presentationRef = useRef(new Compartment());
  // 编辑器自己发出过的内容队列，用于识别父组件回流的旧值（见 external-value.ts）
  const emittedRef = useRef<string[]>([]);
  // 回调统一经 ref 转发，父组件重渲染不会重建编辑器
  const latest = useRef({ onChange, onToggleMode, onOutline, onActiveHeading, resolveImageUrl, t, onUpdate: (_update: ViewUpdate) => {} });
  latest.current = { ...latest.current, onChange, onToggleMode, onOutline, onActiveHeading, resolveImageUrl, t };
  const [view, setView] = useState<EditorView | null>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const bubble = useSelectionBubble(view, live && !readOnly);
  latest.current.onUpdate = (update) => {
    if (update.selectionSet || update.docChanged || update.focusChanged) bubble.refresh();
  };

  useImperativeHandle(ref, () => ({ jumpTo: (from) => viewRef.current && jumpToHeading(viewRef.current, from) }), []);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    // 1. 创建实例：静态扩展一次装配，展示相关扩展放进可热替换的隔间
    const created = new EditorView({
      state: EditorState.create({
        doc: value,
        extensions: [
          ...baseExtensions({
            onChange: (next) => {
              recordEmittedValue(emittedRef.current, next);
              latest.current.onChange(next);
            },
            onUpdate: (update) => latest.current.onUpdate(update),
            keymap: () => ({ onToggleMode: latest.current.onToggleMode, t: latest.current.t }),
            outline: () => ({
              onOutline: (headings) => latest.current.onOutline?.(headings),
              onActive: (index) => latest.current.onActiveHeading?.(index),
            }),
            resolveImageUrl: () => latest.current.resolveImageUrl,
            placeholder: t("Start writing…", "开始输入…"),
          }),
          presentationRef.current.of(presentationExtensions({ live, dark, readOnly, wrap })),
        ],
      }),
      parent: host,
    });
    viewRef.current = created;
    setView(created);
    return () => {
      created.destroy();
      viewRef.current = null;
      setView(null);
    };
    // 仅在挂载时创建实例，内容与配置由后续副作用同步
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    // 2. 模式、主题、只读状态变化时热替换，保留光标与撤销栈
    viewRef.current?.dispatch({
      effects: presentationRef.current.reconfigure(presentationExtensions({ live, dark, readOnly, wrap })),
    });
  }, [live, dark, readOnly, wrap]);

  useEffect(() => {
    const current = viewRef.current;
    if (!current) return;
    const doc = current.state.doc.toString();
    // 3. 编辑器自己发出的内容回流时跳过，避免旧值覆盖更新的文档
    if (resolveExternalValue(value, doc, emittedRef.current).kind !== "replace") return;
    // 4. 真正的外部变化（首次加载、切换文件、磁盘重载）时整体替换：
    //    不进撤销历史，否则一次 Ctrl+Z 就能把文档退回加载前的空白
    const anchor = Math.min(current.state.selection.main.anchor, value.length);
    current.dispatch({
      changes: { from: 0, to: doc.length, insert: value },
      selection: { anchor },
      annotations: Transaction.addToHistory.of(false),
    });
  }, [value]);

  const toolbar = useMemo(() => [blockFormatActions(t), [...inlineFormatActions(t), quoteAction(t)]], [t]);
  const bubbleGroups = useMemo(() => [inlineFormatActions(t), [...blockFormatActions(t).slice(1), quoteAction(t)]], [t]);

  /**
   * 右键打开结构化菜单：顶部为格式图标条，下方为与位置相关的动作。
   *
   * 源码模式与只读状态保留浏览器原生菜单。
   *
   * @param event 右键事件
   * @returns 无
   */
  const openMenu = (event: MouseEvent<HTMLDivElement>) => {
    const current = viewRef.current;
    if (!live || readOnly || !current) return;
    event.preventDefault();
    const cell = cellRefOf(event.target as Element);
    // 1. 正文区域右键时，光标不在选区内才移动光标，保留用户的选区
    if (!cell) {
      const position = current.posAtCoords({ x: event.clientX, y: event.clientY });
      const selection = current.state.selection.main;
      if (position !== null && (position < selection.from || position > selection.to)) {
        current.dispatch({ selection: { anchor: position }, userEvent: "select.pointer" });
      }
      current.focus();
    }
    bubble.hide();
    setMenu({ x: event.clientX, y: event.clientY, entries: buildContextEntries(current, cell, t), inTable: Boolean(cell) });
  };

  const closeMenu = useCallback(() => setMenu(null), []);

  return (
    <div className={live ? "markdown-cm live" : "markdown-cm source"} ref={hostRef} onContextMenu={openMenu}>
      {menu && view && (
        <MarkdownContextMenu
          view={view}
          x={menu.x}
          y={menu.y}
          toolbar={menu.inTable ? [] : toolbar}
          entries={menu.entries}
          label={t("Markdown actions", "Markdown 操作")}
          onClose={closeMenu}
        />
      )}
      {bubble.position && view && !menu && (
        <SelectionBubble view={view} position={bubble.position} groups={bubbleGroups} label={t("Format", "格式")} />
      )}
    </div>
  );
}
