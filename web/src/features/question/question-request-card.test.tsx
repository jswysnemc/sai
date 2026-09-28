import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { PendingQuestion } from "../../api/contracts";
import { QuestionRequestCard } from "./question-request-card";
import { firstUnanswered, initialCustomDrafts, summarizeAnswers } from "./question-card-state";

const pending: PendingQuestion = {
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

const multiStep: PendingQuestion = {
  id: "question-2",
  session_id: "session-1",
  request: {
    questions: [
      { header: "范围", question: "需要覆盖哪些模块", multiple: true, options: [{ label: "前端", description: "" }, { label: "后端", description: "" }] },
      { header: "时间", question: "何时开始", options: [{ label: "现在", description: "" }, { label: "明天", description: "" }] }
    ]
  }
};

const zh = (_en: string, value: string) => value;

describe("QuestionRequestCard", () => {
  it("renders the live question with baseline-aligned radio marks and actions", () => {
    const html = renderToStaticMarkup(<QuestionRequestCard pending={pending} active />);

    expect(html).toContain("需要你的回答");
    expect(html).toContain("请选择下一步处理方式");
    expect(html).toContain("现在完成代码修改");
    expect(html).toContain("is-selected");
    expect(html).toContain("lucide-circle-dot");
    expect(html).toContain("question-option-line icon-label");
    expect(html).toContain("确认");
    expect(html).not.toContain("其他回答");
    expect(html).not.toContain("question-steps");
    expect(html).not.toContain("上一题");
  });

  it("shows step dots and checkbox marks for multi-question multi-select prompts", () => {
    const html = renderToStaticMarkup(<QuestionRequestCard pending={multiStep} active />);

    expect(html.match(/role="tab"/g)).toHaveLength(2);
    expect(html).toContain("1/2");
    expect(html).toContain("上一题");
    expect(html).toContain("下一题");
    expect(html).toContain("lucide-square");
    expect(html).toContain("其他回答");
  });

  it("collapses an answered request into a one-line summary bar", () => {
    const html = renderToStaticMarkup(
      <QuestionRequestCard pending={pending} response={{ status: "answered", data: [["直接修改"]] }} active={false} />
    );

    expect(html).toContain("已回答");
    expect(html).toContain("question-summary-bar");
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain('class="ui-collapse"');
    expect(html).not.toContain("question-request-actions");
  });
});

describe("question card state", () => {
  it("finds the first required question without an answer", () => {
    expect(firstUnanswered(multiStep.request.questions, [["前端"], []])).toBe(1);
    expect(firstUnanswered(multiStep.request.questions, [["前端"], ["现在"]])).toBe(-1);
  });

  it("moves defaults outside the option list into the custom draft", () => {
    const questions = [{ ...pending.request.questions[0], default_answers: ["其他做法"] }];
    expect(initialCustomDrafts(questions)).toEqual(["其他做法"]);
  });

  it("joins multi-select answers with the locale separator", () => {
    expect(summarizeAnswers([["前端", "后端"]], zh)).toEqual(["前端、后端"]);
  });
});
