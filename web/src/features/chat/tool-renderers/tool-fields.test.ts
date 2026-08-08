import { describe, expect, it } from "vitest";
import { parseToolFields } from "./tool-fields";

describe("parseToolFields", () => {
  it("短值按行内字段展示", () => {
    const source = JSON.stringify({ path: "src/main.rs", limit: 40 });
    expect(parseToolFields(source)).toEqual([
      { key: "path", value: "src/main.rs", block: false },
      { key: "limit", value: "40", block: false }
    ]);
  });

  it("超长值改为整块展示", () => {
    const source = JSON.stringify({ content: "x".repeat(100) });
    expect(parseToolFields(source)[0].block).toBe(true);
  });

  it("含换行的值改为整块展示", () => {
    const source = JSON.stringify({ patch: "line one\nline two" });
    expect(parseToolFields(source)[0].block).toBe(true);
  });

  it("对象与数组保留缩进结构", () => {
    const source = JSON.stringify({ files: ["a", "b"] });
    expect(parseToolFields(source)[0].value).toBe('[\n  "a",\n  "b"\n]');
  });

  it("短字段排在长字段之前", () => {
    const source = JSON.stringify({ body: "y".repeat(90), path: "a.rs" });
    expect(parseToolFields(source).map((field) => field.key)).toEqual(["path", "body"]);
  });

  it("null 与布尔值转为字面文本", () => {
    const source = JSON.stringify({ cursor: null, force: true });
    expect(parseToolFields(source)).toEqual([
      { key: "cursor", value: "null", block: false },
      { key: "force", value: "true", block: false }
    ]);
  });

  it("非 JSON 对象返回空列表", () => {
    expect(parseToolFields("just text")).toEqual([]);
    expect(parseToolFields("[1, 2]")).toEqual([]);
  });
});
