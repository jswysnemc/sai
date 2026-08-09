import { describe, expect, it } from "vitest";
import { fieldSelectOptions } from "./structured-config-fields";

describe("structured config finite fields", () => {
  const translate = (_en: string, zh: string) => zh;

  it.each(["reasoning", "tool_calls"])("为 %s 提供显示方式下拉选项", (name) => {
    expect(fieldSelectOptions(name, translate)?.map((option) => option.value)).toEqual([
      "hidden",
      "summary",
      "full"
    ]);
  });

  it("为命令过滤方式提供有限选项", () => {
    expect(fieldSelectOptions("command_filter", translate)?.map((option) => option.value)).toEqual([
      "auto",
      "rtk",
      "off"
    ]);
  });

  it("普通字符串字段继续使用文本输入", () => {
    expect(fieldSelectOptions("output_dir", translate)).toBeNull();
  });
});
