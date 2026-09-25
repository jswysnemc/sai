import { syntaxTree } from "@codemirror/language";
import { EditorSelection, type EditorState, type Line, type StateCommand, type TransactionSpec } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";

/** 列表类型。 */
export type ListKind = "bullet" | "ordered" | "task";

/** 行首引用前缀。 */
const QUOTE = /^(?:[ \t]{0,3}>[ \t]?)*/;

/** 行首列表前缀，含可选任务框。 */
const LIST = /^([ \t]*)(?:[-*+]|\d{1,9}[.)])[ \t]+(?:\[[ xX]\][ \t]+)?/;

/** 行首标题前缀。 */
const HEADING = /^[ \t]{0,3}#{1,6}(?:[ \t]+|$)/;

/**
 * 取选区覆盖到的全部行（去重、按序）。
 *
 * @param state 编辑器状态
 * @returns 行列表
 */
function selectedLines(state: EditorState): Line[] {
  const lines = new Map<number, Line>();
  for (const range of state.selection.ranges) {
    const first = state.doc.lineAt(range.from).number;
    const last = state.doc.lineAt(range.to).number;
    for (let number = first; number <= last; number += 1) lines.set(number, state.doc.line(number));
  }
  return [...lines.values()].sort((left, right) => left.number - right.number);
}

/**
 * 逐行改写选区覆盖的行，选区随改写映射。
 *
 * @param state 编辑器状态
 * @param rewrite 行改写函数：参数为引用前缀之后的正文与行下标，返回新正文
 * @returns 事务说明
 */
function rewriteLines(state: EditorState, rewrite: (body: string, index: number) => string) {
  const changes = selectedLines(state).flatMap((line, index) => {
    const quote = QUOTE.exec(line.text)?.[0] ?? "";
    const body = line.text.slice(quote.length);
    const next = rewrite(body, index);
    return next === body ? [] : [{ from: line.from + quote.length, to: line.to, insert: next }];
  });
  const changeSet = state.changes(changes);
  return { changes: changeSet, selection: state.selection.map(changeSet, 1), userEvent: "input.structure" };
}

/**
 * 设置标题级别；已是该级别时还原为正文，0 表示正文。
 *
 * @param level 标题级别，0 到 6
 * @returns CodeMirror 状态命令
 */
export function setHeading(level: number): StateCommand {
  return ({ state, dispatch }) => {
    const lines = selectedLines(state);
    const same = lines.every((line) => new RegExp(`^[ \\t]{0,3}#{${level}}[ \\t]`).test(line.text.slice((QUOTE.exec(line.text)?.[0] ?? "").length)));
    dispatch(
      state.update(
        rewriteLines(state, (body) => {
          const plain = body.replace(HEADING, "").replace(LIST, "");
          return level === 0 || same ? plain : `${"#".repeat(level)} ${plain}`;
        })
      )
    );
    return true;
  };
}

/**
 * 切换引用：全部行都是引用时去掉一层，否则每行加一层。
 *
 * @returns CodeMirror 状态命令
 */
export function toggleQuote(): StateCommand {
  return ({ state, dispatch }) => {
    const lines = selectedLines(state);
    const quoted = lines.every((line) => /^[ \t]{0,3}>/.test(line.text));
    const changes = lines.map((line) => {
      if (quoted) {
        const mark = /^[ \t]{0,3}>[ \t]?/.exec(line.text)?.[0] ?? "";
        return { from: line.from, to: line.from + mark.length, insert: "" };
      }
      return { from: line.from, insert: "> " };
    });
    const changeSet = state.changes(changes);
    dispatch(state.update({ changes: changeSet, selection: state.selection.map(changeSet, 1), userEvent: "input.structure" }));
    return true;
  };
}

/**
 * 切换列表：全部行已是该类型时去掉列表符号，否则统一改成该类型。
 *
 * @param kind 列表类型
 * @returns CodeMirror 状态命令
 */
export function toggleList(kind: ListKind): StateCommand {
  return ({ state, dispatch }) => {
    const lines = selectedLines(state);
    const matcher = { bullet: /^[ \t]*[-*+][ \t]+(?!\[[ xX]\])/, ordered: /^[ \t]*\d{1,9}[.)][ \t]+/, task: /^[ \t]*[-*+][ \t]+\[[ xX]\]/ }[kind];
    const already = lines.every((line) => matcher.test(line.text.slice((QUOTE.exec(line.text)?.[0] ?? "").length)));
    dispatch(
      state.update(
        rewriteLines(state, (body, index) => {
          const indent = /^[ \t]*/.exec(body)?.[0] ?? "";
          const plain = body.replace(LIST, "").replace(HEADING, "").trimStart();
          if (already) return `${indent}${plain}`;
          const marker = kind === "bullet" ? "- " : kind === "task" ? "- [ ] " : `${index + 1}. `;
          return `${indent}${marker}${plain}`;
        })
      )
    );
    return true;
  };
}

/**
 * 在当前位置插入一个块：当前行为空时就地替换，否则插在当前行之后并隔一个空行。
 *
 * 块后总保证有一个空行或正文行，避免表格把紧随其后的段落吞成数据行，
 * 也让插在文末的块之后仍有地方继续输入。
 *
 * @param state 编辑器状态
 * @param block 块源码，不含首尾换行
 * @param cursorOffset 插入后光标相对块起点的偏移
 * @returns 事务说明与块起点
 */
export function insertBlock(state: EditorState, block: string, cursorOffset: number): { spec: TransactionSpec; blockFrom: number } {
  const doc = state.doc;
  const line = doc.lineAt(state.selection.main.head);
  const empty = line.text.trim() === "";
  const from = empty ? line.from : line.to;
  const previous = line.number > 1 ? doc.line(line.number - 1) : null;
  // 块前同样需要空行：分隔线紧跟段落会被解析成 Setext 标题
  const prefix = !empty ? "\n\n" : previous?.text.trim() ? "\n" : "";
  const next = line.number < doc.lines ? doc.line(line.number + 1) : null;
  const suffix = !next || next.text.trim() ? "\n" : "";
  const blockFrom = from + prefix.length;
  return {
    spec: {
      changes: { from, to: empty ? line.to : from, insert: `${prefix}${block}${suffix}` },
      selection: EditorSelection.cursor(blockFrom + cursorOffset),
      scrollIntoView: true,
      userEvent: "input.structure",
    },
    blockFrom,
  };
}

/**
 * 插入围栏代码块；有选区时把选中行包进代码块。
 *
 * @param language 语言标识
 * @returns CodeMirror 状态命令
 */
export function insertCodeBlock(language = ""): StateCommand {
  return ({ state, dispatch }) => {
    const range = state.selection.main;
    if (!range.empty) {
      const first = state.doc.lineAt(range.from);
      const last = state.doc.lineAt(range.to);
      const body = state.doc.sliceString(first.from, last.to);
      const insert = `\`\`\`${language}\n${body}\n\`\`\``;
      dispatch(
        state.update({
          changes: { from: first.from, to: last.to, insert },
          selection: EditorSelection.cursor(first.from + language.length + 4),
          userEvent: "input.structure",
        })
      );
      return true;
    }
    const opening = `\`\`\`${language}\n`;
    dispatch(state.update(insertBlock(state, `${opening}\n\`\`\``, opening.length).spec));
    return true;
  };
}

/**
 * 插入块级公式。
 *
 * @returns CodeMirror 状态命令
 */
export function insertMathBlock(): StateCommand {
  return ({ state, dispatch }) => {
    dispatch(state.update(insertBlock(state, "$$\n\n$$", 3).spec));
    return true;
  };
}

/**
 * 插入分隔线。
 *
 * @returns CodeMirror 状态命令
 */
export function insertRule(): StateCommand {
  return ({ state, dispatch }) => {
    dispatch(state.update(insertBlock(state, "---", 4).spec));
    return true;
  };
}

/**
 * 找到光标所在的围栏代码块。
 *
 * @param state 编辑器状态
 * @param position 参考位置，缺省为主光标
 * @returns FencedCode 节点；不在代码块内时为 null
 */
export function fencedCodeAt(state: EditorState, position = state.selection.main.head): SyntaxNode | null {
  for (const side of [1, -1] as const) {
    for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(position, side); node; node = node.parent) {
      if (node.name === "FencedCode") return node;
    }
  }
  return null;
}

/**
 * 切换光标所在代码块的语言。
 *
 * @param language 新语言标识，空串表示纯文本
 * @returns CodeMirror 状态命令；不在代码块内时返回 false
 */
export function setCodeLanguage(language: string): StateCommand {
  return ({ state, dispatch }) => {
    const block = fencedCodeAt(state);
    if (!block) return false;
    const openLine = state.doc.lineAt(block.from);
    const fence = /^[ \t]*(`{3,}|~{3,})/.exec(openLine.text);
    if (!fence) return false;
    const from = openLine.from + fence[0].length;
    dispatch(state.update({ changes: { from, to: openLine.to, insert: language }, userEvent: "input.structure" }));
    return true;
  };
}

/**
 * 读取光标所在代码块的语言。
 *
 * @param state 编辑器状态
 * @returns 语言标识；不在代码块内时为 null
 */
export function codeLanguageAt(state: EditorState): string | null {
  const block = fencedCodeAt(state);
  if (!block) return null;
  const info = block.getChild("CodeInfo");
  return info ? state.doc.sliceString(info.from, info.to).trim() : "";
}
