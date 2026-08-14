import { describe, expect, it } from "vitest";
import { formatDuration, formatTokens, prettyJson, summarizeContent, summarizeToolArguments } from "./trajectory-format";

describe("summarizeContent", () => {
  it("把多行内容压成单行", () => {
    expect(summarizeContent("第一行\n\n  第二行  ")).toBe("第一行 第二行");
  });

  it("超长内容截断并加省略号", () => {
    const summary = summarizeContent("a".repeat(400));
    expect(summary.endsWith("…")).toBe(true);
    expect(summary.length).toBe(221);
  });
});

describe("summarizeToolArguments", () => {
  it("把 JSON 入参展开成键值序列", () => {
    expect(summarizeToolArguments('{"path":"src/main.rs","limit":40}'))
      .toBe("path=src/main.rs · limit=40");
  });

  it("嵌套值收敛为占位符而不是展开", () => {
    expect(summarizeToolArguments('{"items":[1,2,3],"opts":{"a":1}}'))
      .toBe("items=[3] · opts={…}");
  });

  it("非 JSON 入参原样压成单行", () => {
    expect(summarizeToolArguments("ls -la\n/tmp")).toBe("ls -la /tmp");
  });

  it("空入参返回空串", () => {
    expect(summarizeToolArguments("   ")).toBe("");
  });
});

describe("formatDuration", () => {
  it("毫秒级保留整数毫秒", () => {
    expect(formatDuration(842)).toBe("842ms");
  });

  it("秒级保留两位小数", () => {
    expect(formatDuration(1500)).toBe("1.50s");
  });

  it("十秒以上只保留一位小数", () => {
    expect(formatDuration(12_340)).toBe("12.3s");
  });

  it("超过一分钟拆成分秒", () => {
    expect(formatDuration(185_000)).toBe("3m05s");
  });

  it("未知耗时返回短横线而不是零", () => {
    expect(formatDuration(null)).toBe("-");
  });
});

describe("formatTokens", () => {
  it("千以下原样显示", () => {
    expect(formatTokens(940)).toBe("940");
  });

  it("千到万之间保留一位小数", () => {
    expect(formatTokens(3520)).toBe("3.5k");
  });

  it("万以上取整", () => {
    expect(formatTokens(148_320)).toBe("148k");
  });
});

describe("prettyJson", () => {
  it("格式化合法 JSON", () => {
    expect(prettyJson('{"a":1}')).toBe("{\n  \"a\": 1\n}");
  });

  it("非 JSON 原样返回", () => {
    expect(prettyJson("not json")).toBe("not json");
  });
});
