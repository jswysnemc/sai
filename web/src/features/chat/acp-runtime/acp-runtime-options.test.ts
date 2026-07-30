import { describe, expect, it } from "vitest";
import type { EngineStatusResponse } from "../../../api/contracts";
import { acpAdjustableOptions, parseAcpRuntimeOptions } from "./acp-runtime-options";

/**
 * 构造带指定配置项的内核状态。
 *
 * @param configOptions agent 上报的配置项
 * @returns 内核运行状态
 */
function status(configOptions: unknown): EngineStatusResponse {
  return {
    engine: "claude_code",
    label: "Claude Code",
    external: true,
    unavailable_features: [],
    acp_runtime: { config_options: configOptions } as never
  } as EngineStatusResponse;
}

describe("acp runtime options", () => {
  it("parses select and boolean options", () => {
    const options = parseAcpRuntimeOptions([
      {
        id: "mode",
        name: "Permission mode",
        category: "mode",
        type: "select",
        currentValue: "ask",
        options: [{ value: "ask", name: "Ask" }, { value: "auto", name: "Auto" }]
      },
      { id: "verbose", name: "Verbose", type: "boolean", currentValue: false }
    ]);

    expect(options).toHaveLength(2);
    expect(options[0]?.values.map((item) => item.value)).toEqual(["ask", "auto"]);
    expect(options[1]?.type).toBe("boolean");
  });

  it("flattens grouped select options", () => {
    const options = parseAcpRuntimeOptions([
      {
        id: "model",
        name: "Model",
        type: "select",
        currentValue: "a",
        options: [{ options: [{ value: "a", name: "A" }, { value: "b", name: "B" }] }]
      }
    ]);

    expect(options[0]?.values.map((item) => item.value)).toEqual(["a", "b"]);
  });

  it("drops malformed and empty entries", () => {
    expect(parseAcpRuntimeOptions(null)).toEqual([]);
    expect(
      parseAcpRuntimeOptions([
        null,
        { id: "no-name", type: "boolean", currentValue: true },
        { id: "empty", name: "Empty", type: "select", currentValue: "x", options: [] }
      ])
    ).toEqual([]);
  });

  it("excludes categories that already have dedicated composer controls", () => {
    const options = acpAdjustableOptions(
      status([
        { id: "model", name: "Model", category: "model", type: "select", currentValue: "a", options: [{ value: "a", name: "A" }] },
        { id: "thought", name: "Thinking", category: "thought_level", type: "select", currentValue: "low", options: [{ value: "low", name: "Low" }] },
        { id: "mode", name: "Mode", category: "mode", type: "select", currentValue: "ask", options: [{ value: "ask", name: "Ask" }] },
        { id: "verbose", name: "Verbose", type: "boolean", currentValue: false }
      ])
    );

    // 模型与思考等级由专用选择器承担，权限模式与自定义项留给弹层
    expect(options.map((option) => option.id)).toEqual(["mode", "verbose"]);
  });

  it("returns nothing without a handshake", () => {
    expect(acpAdjustableOptions(undefined)).toEqual([]);
  });
});
