import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { QuestionRequestCard } from "./question-request-card";

const pending = {
  id: "question-1",
  session_id: "session-1",
  request: {
    questions: [{
      header: "实现方式",
      question: "请选择下一步处理方式",
      options: [
        { label: "直接修改", description: "现在完成代码修改" },
        { label: "先给方案", description: "确认方案后再修改" }
      ],
      custom: false,
      default_answers: ["直接修改"]
    }]
  }
};

describe("QuestionRequestCard", () => {
  it("renders compact numbered options and a confirmation footer", () => {
    const html = renderToStaticMarkup(<QuestionRequestCard pending={pending} active />);

    expect(html).toContain("需要你的回答");
    expect(html).toContain("question-option-index");
    expect(html).toContain("1.");
    expect(html).toContain("直接修改");
    expect(html).toContain("现在完成代码修改");
    expect(html).toContain("使用 Tab / 上下键移动，回车或空格选中");
    expect(html).toContain("确认");
    expect(html).toContain("is-selected");
  });

  it("renders an answered request without active controls", () => {
    const html = renderToStaticMarkup(
      <QuestionRequestCard pending={pending} response={{ status: "answered", data: [["直接修改"]] }} active={false} />
    );

    expect(html).toContain("已回答");
    expect(html).not.toContain("question-request-footer");
  });
});
