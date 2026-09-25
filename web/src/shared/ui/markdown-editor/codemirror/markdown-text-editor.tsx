import { Compartment, EditorState, Transaction } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { useEffect, useRef, useState, type MouseEvent } from "react";
import { ContextActionMenu } from "../../menu/context-action-menu";
import type { ActionMenuItem } from "../../menu/action-menu";
import { baseExtensions, presentationExtensions } from "./editor-extensions";
import { recordEmittedValue, resolveExternalValue } from "./external-value";
import {
  changeTableColumn,
  deleteTableRow,
  editTransaction,
  insertCodeBlock,
  insertTable,
  insertTableRow,
  setCodeLanguage,
  setHeading,
  setQuote,
  toggleList,
  wrapSelection,
} from "./wysiwyg-edit";
import "./markdown-text-editor.css";

type MarkdownTextEditorProps = {
  value: string;
  onChange: (value: string) => void;
  /** true 为所见即所得，false 为源码 */
  live: boolean;
  dark: boolean;
  readOnly?: boolean;
  /** 是否自动换行，缺省为换行 */
  wrap?: boolean;
};

/**
 * 基于 CodeMirror 6 的 Markdown 文本编辑器。
 *
 * 文档本身始终是 Markdown 源码，所见即所得靠装饰层隐藏语法标记实现，
 * 因此模式之间往返不会改写用户的原始写法——这是与富文本方案的关键差别。
 *
 * @param props 文档内容、变更回调、模式、主题深浅、只读状态与换行
 * @returns 编辑器容器
 */
export function MarkdownTextEditor({
  value,
  onChange,
  live,
  dark,
  readOnly = false,
  wrap = true,
}: MarkdownTextEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  const presentationRef = useRef(new Compartment());
  // 编辑器自己发出过的内容队列，用于识别父组件回流的旧值（见 external-value.ts）
  const emittedRef = useRef<string[]>([]);
  const [menu, setMenu] = useState<{ x: number; y: number; items: ActionMenuItem[] } | null>(null);
  onChangeRef.current = onChange;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    // 1. 变更回调经 ref 转发，父组件重渲染不会重建编辑器
    const view = new EditorView({
      state: EditorState.create({
        doc: value,
        extensions: [
          ...baseExtensions((next) => {
            recordEmittedValue(emittedRef.current, next);
            onChangeRef.current(next);
          }),
          presentationRef.current.of(presentationExtensions({ live, dark, readOnly, wrap })),
        ],
      }),
      parent: host,
    });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // 仅在挂载时创建实例，内容与配置由后续副作用同步
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    // 2. 模式、主题、只读状态变化时热替换，保留光标与撤销栈
    view.dispatch({
      effects: presentationRef.current.reconfigure(presentationExtensions({ live, dark, readOnly, wrap })),
    });
  }, [live, dark, readOnly, wrap]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = view.state.doc.toString();
    // 3. 编辑器自己发出的内容回流时跳过，避免旧值覆盖更新的文档
    if (resolveExternalValue(value, current, emittedRef.current).kind !== "replace") return;
    // 4. 真正的外部变化（首次加载、切换文件、磁盘重载）时整体替换：
    //    不进撤销历史，否则一次 Ctrl+Z 就能把文档退回加载前的空白
    const anchor = Math.min(view.state.selection.main.anchor, value.length);
    view.dispatch({
      changes: { from: 0, to: current.length, insert: value },
      selection: { anchor },
      annotations: Transaction.addToHistory.of(false),
    });
  }, [value]);

  /**
   * 在所见即所得模式下用右键菜单改结构，避免把整行切回源码。
   *
   * @param event 右键事件
   */
  const openMenu = (event: MouseEvent<HTMLDivElement>) => {
    if (!live || readOnly) return;
    const view = viewRef.current;
    if (!view) return;
    event.preventDefault();
    const position = view.posAtCoords({ x: event.clientX, y: event.clientY });
    if (position != null) view.dispatch({ selection: { anchor: position } });
    const apply = (edit: ReturnType<typeof setHeading> | null) => {
      if (!edit) return;
      view.dispatch(editTransaction(edit));
      view.focus();
    };
    const items: ActionMenuItem[] = [
      { id: "h1", label: "标题 1", onSelect: () => apply(setHeading(view.state, 1)) },
      { id: "h2", label: "标题 2", onSelect: () => apply(setHeading(view.state, 2)) },
      { id: "h3", label: "标题 3", onSelect: () => apply(setHeading(view.state, 3)) },
      { id: "quote-more", label: "提高引用级别", separator: true, onSelect: () => apply(setQuote(view.state, "more")) },
      { id: "quote-less", label: "降低引用级别", onSelect: () => apply(setQuote(view.state, "less")) },
      { id: "bold", label: "加粗", separator: true, onSelect: () => apply(wrapSelection(view.state, "**")) },
      { id: "italic", label: "斜体", onSelect: () => apply(wrapSelection(view.state, "*")) },
      { id: "bullet", label: "无序列表", onSelect: () => apply(toggleList(view.state, "bullet")) },
      { id: "ordered", label: "有序列表", onSelect: () => apply(toggleList(view.state, "ordered")) },
      { id: "table", label: "插入表格", separator: true, onSelect: () => apply(insertTable(view.state)) },
      { id: "row-below", label: "在下方插入行", onSelect: () => apply(insertTableRow(view.state, "below")) },
      { id: "row-above", label: "在上方插入行", onSelect: () => apply(insertTableRow(view.state, "above")) },
      { id: "row-delete", label: "删除行", danger: true, onSelect: () => apply(deleteTableRow(view.state)) },
      { id: "col-add", label: "在右侧插入列", onSelect: () => apply(changeTableColumn(view.state, "add")) },
      { id: "col-remove", label: "删除列", danger: true, onSelect: () => apply(changeTableColumn(view.state, "remove")) },
      { id: "code", label: "插入代码块", separator: true, onSelect: () => apply(insertCodeBlock(view.state, "ts")) },
      { id: "lang-ts", label: "代码语言：TypeScript", onSelect: () => apply(setCodeLanguage(view.state, "ts")) },
      { id: "lang-bash", label: "代码语言：Bash", onSelect: () => apply(setCodeLanguage(view.state, "bash")) },
      { id: "lang-py", label: "代码语言：Python", onSelect: () => apply(setCodeLanguage(view.state, "python")) },
    ];
    setMenu({ x: event.clientX, y: event.clientY, items });
  };

  return (
    <div className={live ? "markdown-cm live" : "markdown-cm source"} ref={hostRef} onContextMenu={openMenu}>
      {menu && (
        <ContextActionMenu
          label="Markdown"
          x={menu.x}
          y={menu.y}
          items={menu.items}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}
