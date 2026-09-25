import type { EditorView } from "@codemirror/view";
import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  ArrowDownToLine,
  ArrowLeftToLine,
  ArrowRightToLine,
  ArrowUpToLine,
  Braces,
  ClipboardCopy,
  Code2,
  Columns3,
  List,
  ListOrdered,
  ListTodo,
  Minus,
  Plus,
  Quote,
  Rows3,
  Scissors,
  Sigma,
  Table,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import { modKeyLabel } from "../../../../mod-key";
import { codeLanguageAt, fencedCodeAt, insertCodeBlock, insertMathBlock, insertRule, setCodeLanguage, toggleList, toggleQuote } from "../commands/block-commands";
import type { Translate } from "../editor-format-actions";
import { deleteTable, editTable, insertTable } from "../wysiwyg-table-actions";
import { deleteColumn, deleteRow, insertColumn, insertRow, setColumnAlign } from "../wysiwyg-table-edit";
import type { TableCellRef } from "../wysiwyg-table-focus";
import type { TableAlign } from "../wysiwyg-table-model";

/** 菜单项；带 children 时为子菜单。 */
export type MenuEntry = {
  id: string;
  label: string;
  icon?: LucideIcon;
  shortcut?: string;
  danger?: boolean;
  disabled?: boolean;
  checked?: boolean;
  /** 与上一项之间画分隔线 */
  separator?: boolean;
  run?: () => void;
  children?: MenuEntry[];
};

/** 代码块语言候选，覆盖文档里最常见的几种。 */
const CODE_LANGUAGES = [
  ["", "Plain text"],
  ["ts", "TypeScript"],
  ["tsx", "TSX"],
  ["js", "JavaScript"],
  ["json", "JSON"],
  ["bash", "Bash"],
  ["python", "Python"],
  ["rust", "Rust"],
  ["go", "Go"],
  ["java", "Java"],
  ["sql", "SQL"],
  ["yaml", "YAML"],
  ["html", "HTML"],
  ["css", "CSS"],
  ["markdown", "Markdown"],
  ["mermaid", "Mermaid"],
] as const;

/**
 * 按右键位置构建上下文菜单项。
 *
 * 表格单元格、代码块与普通正文各有一组动作，菜单只展示与当前位置相关的部分，
 * 通用的格式动作放在菜单顶部的图标条里，不在列表中重复。
 *
 * @param view 编辑器视图
 * @param cell 右键所在的表格单元格；不在表格内时为 null
 * @param t 双语文案取值函数
 * @returns 菜单项列表
 */
export function buildContextEntries(view: EditorView, cell: TableCellRef | null, t: Translate): MenuEntry[] {
  const entries: MenuEntry[] = [];
  const selection = view.state.selection.main;
  // 1. 有选区时提供剪切与复制
  if (!cell && !selection.empty) {
    const text = view.state.sliceDoc(selection.from, selection.to);
    entries.push(
      {
        id: "cut",
        label: t("Cut", "剪切"),
        icon: Scissors,
        shortcut: `${modKeyLabel()}+X`,
        run: () => {
          void navigator.clipboard?.writeText(text);
          view.dispatch({ changes: { from: selection.from, to: selection.to }, userEvent: "delete.cut" });
        },
      },
      { id: "copy", label: t("Copy", "复制"), icon: ClipboardCopy, shortcut: `${modKeyLabel()}+C`, run: () => void navigator.clipboard?.writeText(text) }
    );
  }
  // 2. 按位置追加专属动作
  if (cell) entries.push(...tableEntries(view, cell, t));
  else if (fencedCodeAt(view.state)) entries.push(...codeEntries(view, t));
  else entries.push(...blockEntries(view, t));
  if (entries.length && entries[0].separator) entries[0] = { ...entries[0], separator: false };
  return entries;
}

/**
 * 普通正文的块级动作：引用、列表与插入。
 *
 * @param view 编辑器视图
 * @param t 双语文案取值函数
 * @returns 菜单项列表
 */
function blockEntries(view: EditorView, t: Translate): MenuEntry[] {
  const mod = modKeyLabel();
  return [
    { id: "quote", label: t("Quote", "引用"), icon: Quote, shortcut: `${mod}+Shift+Q`, separator: true, run: () => toggleQuote()(view) },
    { id: "bullet", label: t("Bulleted list", "无序列表"), icon: List, shortcut: `${mod}+Shift+]`, run: () => toggleList("bullet")(view) },
    { id: "ordered", label: t("Numbered list", "有序列表"), icon: ListOrdered, shortcut: `${mod}+Shift+[`, run: () => toggleList("ordered")(view) },
    { id: "task", label: t("Task list", "任务列表"), icon: ListTodo, run: () => toggleList("task")(view) },
    {
      id: "insert",
      label: t("Insert", "插入"),
      icon: Plus,
      separator: true,
      children: [
        {
          id: "table",
          label: t("Table", "表格"),
          icon: Table,
          shortcut: `${mod}+Alt+T`,
          run: () => insertTable(view, 3, 2, (index) => t(`Column ${index + 1}`, `列 ${index + 1}`)),
        },
        { id: "code", label: t("Code block", "代码块"), icon: Code2, shortcut: `${mod}+Shift+K`, run: () => insertCodeBlock()(view) },
        { id: "math", label: t("Math block", "公式块"), icon: Sigma, shortcut: `${mod}+Shift+M`, run: () => insertMathBlock()(view) },
        { id: "mermaid", label: t("Mermaid diagram", "Mermaid 图表"), icon: Braces, run: () => insertCodeBlock("mermaid")(view) },
        { id: "rule", label: t("Divider", "分隔线"), icon: Minus, run: () => insertRule()(view) },
      ],
    },
  ];
}

/**
 * 代码块内的动作：切换语言、复制与删除。
 *
 * @param view 编辑器视图
 * @param t 双语文案取值函数
 * @returns 菜单项列表
 */
function codeEntries(view: EditorView, t: Translate): MenuEntry[] {
  const current = codeLanguageAt(view.state) ?? "";
  const block = fencedCodeAt(view.state);
  return [
    {
      id: "language",
      label: t("Language", "代码语言"),
      icon: Code2,
      children: CODE_LANGUAGES.map(([id, name]) => ({
        id: `lang-${id || "plain"}`,
        label: id ? name : t("Plain text", "纯文本"),
        checked: current.toLowerCase() === id,
        run: () => setCodeLanguage(id)(view),
      })),
    },
    {
      id: "copy-code",
      label: t("Copy code", "复制代码"),
      icon: ClipboardCopy,
      run: () => {
        if (!block) return;
        const lines = view.state.sliceDoc(block.from, block.to).split("\n");
        void navigator.clipboard?.writeText(lines.slice(1, /^\s*(`{3,}|~{3,})\s*$/.test(lines[lines.length - 1]) ? -1 : undefined).join("\n"));
      },
    },
    {
      id: "delete-code",
      label: t("Delete code block", "删除代码块"),
      icon: Trash2,
      danger: true,
      separator: true,
      run: () => {
        if (block) view.dispatch({ changes: { from: block.from, to: block.to }, selection: { anchor: block.from }, userEvent: "input.structure" });
      },
    },
  ];
}

/**
 * 表格单元格内的动作：增删行列、列对齐与删除表格。
 *
 * @param view 编辑器视图
 * @param cell 单元格坐标
 * @param t 双语文案取值函数
 * @returns 菜单项列表
 */
function tableEntries(view: EditorView, cell: TableCellRef, t: Translate): MenuEntry[] {
  const { tableFrom, row, column } = cell;
  const dataIndex = row - 1;
  const focus = (nextRow: number, nextColumn: number) => ({ row: nextRow, column: nextColumn, caret: "start" as const });
  const align = (value: TableAlign) => () => editTable(view, tableFrom, (matrix) => setColumnAlign(matrix, column, value), focus(row, column));
  return [
    { id: "row-above", label: t("Insert row above", "在上方插入行"), icon: ArrowUpToLine, disabled: row === 0, run: () => editTable(view, tableFrom, (matrix) => insertRow(matrix, dataIndex), focus(row, column)) },
    { id: "row-below", label: t("Insert row below", "在下方插入行"), icon: ArrowDownToLine, shortcut: `${modKeyLabel()}+Enter`, run: () => editTable(view, tableFrom, (matrix) => insertRow(matrix, row), focus(row + 1, column)) },
    { id: "row-delete", label: t("Delete row", "删除行"), icon: Rows3, danger: true, disabled: row === 0, run: () => editTable(view, tableFrom, (matrix) => deleteRow(matrix, dataIndex), focus(Math.max(0, row - 1), column)) },
    { id: "col-left", label: t("Insert column left", "在左侧插入列"), icon: ArrowLeftToLine, separator: true, run: () => editTable(view, tableFrom, (matrix) => insertColumn(matrix, column), focus(row, column)) },
    { id: "col-right", label: t("Insert column right", "在右侧插入列"), icon: ArrowRightToLine, run: () => editTable(view, tableFrom, (matrix) => insertColumn(matrix, column + 1), focus(row, column + 1)) },
    { id: "col-delete", label: t("Delete column", "删除列"), icon: Columns3, danger: true, run: () => editTable(view, tableFrom, (matrix) => deleteColumn(matrix, column), focus(row, Math.max(0, column - 1))) },
    {
      id: "align",
      label: t("Column alignment", "列对齐"),
      icon: AlignLeft,
      separator: true,
      children: [
        { id: "align-left", label: t("Left", "左对齐"), icon: AlignLeft, run: align("left") },
        { id: "align-center", label: t("Center", "居中"), icon: AlignCenter, run: align("center") },
        { id: "align-right", label: t("Right", "右对齐"), icon: AlignRight, run: align("right") },
        { id: "align-none", label: t("Default", "默认"), run: align(null) },
      ],
    },
    { id: "table-delete", label: t("Delete table", "删除表格"), icon: Trash2, danger: true, separator: true, run: () => deleteTable(view, tableFrom) },
  ];
}
