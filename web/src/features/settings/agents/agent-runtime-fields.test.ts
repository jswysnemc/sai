import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import { buildAgentModelChoices } from "../../agents/agent-runtime-options";

const config = {
  active_provider: "active",
  providers: [
    { id: "active", display_name: "当前供应商", models: ["active-model"] },
    { id: "child", display_name: "子供应商", models: ["child-model"] }
  ]
} as AppConfig;

describe("buildAgentModelChoices", () => {
  it("生成供应商和模型组合选项", () => {
    expect(buildAgentModelChoices(config, "", "").map((choice) => choice.value))
      .toEqual(["active\tactive-model", "child\tchild-model"]);
  });

  it("保留模型列表外的历史覆盖值", () => {
    expect(buildAgentModelChoices(config, "legacy", "legacy-model")[0]).toMatchObject({
      value: "legacy\tlegacy-model",
      label: "legacy / legacy-model"
    });
  });
});
