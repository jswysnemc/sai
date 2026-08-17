import { describe, expect, it } from "vitest";
import { parseAutomaticInput } from "./parse-automatic-input";

describe("parseAutomaticInput", () => {
  it("拆出标题和后台命令字段，避免收成一段长文", () => {
    const model = parseAutomaticInput([
      "后台工作已完成，自动继续当前对话",
      "",
      "后台命令：find /home/snemc -name \"call_00_dlKAoiWbXX8qxDmd\"（1786994714-794141）",
      "状态：exited",
      "说明：日志未附带，请使用 background_command action=output 读取（默认前 50 行，可用 tail_lines 调整）"
    ].join("\n"));

    expect(model.title).toBe("后台工作已完成，自动继续当前对话");
    expect(model.notices).toEqual([
      {
        fields: [
          { label: "后台命令", value: "find /home/snemc -name \"call_00_dlKAoiWbXX8qxDmd\"（1786994714-794141）" },
          { label: "状态", value: "exited" },
          { label: "说明", value: "日志未附带，请使用 background_command action=output 读取（默认前 50 行，可用 tail_lines 调整）" }
        ],
        leftover: ""
      }
    ]);
  });

  it("没有字段时只保留标题", () => {
    expect(parseAutomaticInput("后台任务已完成")).toEqual({
      title: "后台任务已完成",
      notices: []
    });
  });
});
