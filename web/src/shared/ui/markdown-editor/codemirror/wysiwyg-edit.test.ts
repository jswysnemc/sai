import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { insertTableRow, setCodeLanguage, setHeading, setQuote } from "./wysiwyg-edit";

function stateOf(doc: string, cursor: number) {
  return EditorState.create({
    doc,
    selection: { anchor: cursor },
    extensions: [markdown({ base: markdownLanguage })],
  });
}

describe("wysiwyg edits", () => {
  it("把当前行改成标题且光标行不需要先露出标记", () => {
    const state = stateOf("正文", 0);
    expect(setHeading(state, 2).insert).toBe("## 正文");
  });

  it("提高引用级别", () => {
    const state = stateOf("> 说明", 3);
    expect(setQuote(state, "more").insert).toBe("> > 说明");
  });

  it("在表格行下方插入空行", () => {
    const doc = "| a | b |\n| --- | --- |\n| 1 | 2 |";
    const state = stateOf(doc, doc.length - 2);
    const edit = insertTableRow(state, "below");
    expect(edit?.insert).toBe("\n|  |  |");
  });

  it("切换代码块语言", () => {
    const doc = "```ts\nconst a = 1;\n```";
    const state = stateOf(doc, 8);
    expect(setCodeLanguage(state, "bash")?.insert).toBe("```bash");
  });
});
