import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorSelection, EditorState, type StateCommand } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { markdownMath } from "../markdown-math-syntax";
import { insertBlock, setCodeLanguage, setHeading, toggleList, toggleQuote } from "./block-commands";
import { BOLD, insertLink, toggleInline } from "./inline-commands";
import { smartBackspace, smartEnter, smartTab } from "./structure-keys";

/**
 * 构建带选区的编辑器状态，语法树预先解析完整。
 *
 * @param doc 文档内容
 * @param anchor 选区锚点
 * @param head 选区头，缺省与锚点相同
 * @returns 编辑器状态
 */
function stateOf(doc: string, anchor: number, head = anchor) {
  const state = EditorState.create({
    doc,
    selection: EditorSelection.single(anchor, head),
    extensions: [markdown({ base: markdownLanguage, extensions: [markdownMath] })],
  });
  ensureSyntaxTree(state, state.doc.length, 1000);
  return state;
}

/**
 * 执行命令并返回结果文档与主光标。
 *
 * @param state 初始状态
 * @param command 状态命令
 * @returns 是否处理、结果文档与主光标位置
 */
function run(state: EditorState, command: StateCommand) {
  let next = state;
  const handled = command({ state, dispatch: (transaction) => (next = transaction.state) });
  return { handled, doc: next.doc.toString(), head: next.selection.main.head };
}

describe("行内格式", () => {
  it("有选区时包裹，已加粗时去掉标记", () => {
    expect(run(stateOf("重点", 0, 2), toggleInline(BOLD)).doc).toBe("**重点**");
    expect(run(stateOf("**重点**", 3), toggleInline(BOLD)).doc).toBe("重点");
  });

  it("无选区时插入空标记并把光标放在中间", () => {
    const result = run(stateOf("", 0), toggleInline(BOLD));
    expect(result.doc).toBe("****");
    expect(result.head).toBe(2);
  });

  it("选中文字粘贴网址生成链接", () => {
    expect(run(stateOf("文档", 0, 2), insertLink("https://a.b")).doc).toBe("[文档](https://a.b)");
  });
});

describe("块级格式", () => {
  it("设置标题，同级再按一次还原为正文", () => {
    expect(run(stateOf("正文", 0), setHeading(2)).doc).toBe("## 正文");
    expect(run(stateOf("## 正文", 4), setHeading(2)).doc).toBe("正文");
    expect(run(stateOf("### 正文", 4), setHeading(0)).doc).toBe("正文");
  });

  it("切换引用", () => {
    expect(run(stateOf("说明", 0), toggleQuote()).doc).toBe("> 说明");
    expect(run(stateOf("> 说明", 3), toggleQuote()).doc).toBe("说明");
  });

  it("切换列表并为有序列表编号", () => {
    expect(run(stateOf("a\nb", 0, 3), toggleList("ordered")).doc).toBe("1. a\n2. b");
    expect(run(stateOf("- a", 2), toggleList("bullet")).doc).toBe("a");
    expect(run(stateOf("- a", 2), toggleList("task")).doc).toBe("- [ ] a");
  });

  it("插入块时与前后段落各隔一个空行", () => {
    const { spec } = insertBlock(stateOf("段落\n后文", 1), "---", 4);
    const state = stateOf("段落\n后文", 1).update(spec).state;
    expect(state.doc.toString()).toBe("段落\n\n---\n\n后文");
  });

  it("切换代码块语言", () => {
    expect(run(stateOf("```ts\nconst a = 1;\n```", 8), setCodeLanguage("bash")).doc).toBe("```bash\nconst a = 1;\n```");
  });
});

describe("结构键", () => {
  it("在未闭合的围栏行尾回车时补全闭栏", () => {
    const result = run(stateOf("```ts", 5), smartEnter());
    expect(result.doc).toBe("```ts\n\n```");
    expect(result.head).toBe(6);
  });

  it("在 $$ 行尾回车时补全公式块", () => {
    expect(run(stateOf("$$", 2), smartEnter()).doc).toBe("$$\n\n$$");
  });

  it("在标题正文开头回车时在上方插入空行", () => {
    const result = run(stateOf("## 标题", 3), smartEnter());
    expect(result.doc).toBe("\n## 标题");
    expect(result.head).toBe(4);
  });

  it("在标题正文开头退格时降为正文", () => {
    expect(run(stateOf("## 标题", 3), smartBackspace()).doc).toBe("标题");
  });

  it("空代码块内退格时整块删除", () => {
    expect(run(stateOf("```\n\n```", 4), smartBackspace()).doc).toBe("");
  });

  it("有内容的代码块开头退格不合并围栏", () => {
    const result = run(stateOf("```\ncode\n```", 4), smartBackspace());
    expect(result.handled).toBe(true);
    expect(result.doc).toBe("```\ncode\n```");
  });

  it("Tab 按上一个同级项的正文起点缩进，Shift-Tab 还原", () => {
    expect(run(stateOf("1. a\n2. b", 8), smartTab(false)).doc).toBe("1. a\n   2. b");
    expect(run(stateOf("- a\n  - b", 8), smartTab(true)).doc).toBe("- a\n- b");
  });
});
