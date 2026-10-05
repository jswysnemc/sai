import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../api/contracts";
import { buildChatModelChoices, groupChatModelChoices, resolveChatModelSelection, shortModelName } from "./chat-model-options";

const config: AppConfig = {
  active_provider: "primary",
  gateways: {} as AppConfig["gateways"],
  providers: [
    { id: "primary", display_name: "Primary", base_url: "", models: ["model-a", "model-b"], default_model: "model-b" },
    { id: "backup", display_name: "Backup", base_url: "", models: [], default_model: "model-c" }
  ]
};

describe("chat model options", () => {
  it("uses provider models and default model fallbacks", () => {
    expect(buildChatModelChoices(config).map((choice) => choice.model)).toEqual(["model-a", "model-b", "model-c"]);
  });

  it("prefers the saved choice when it remains valid", () => {
    expect(resolveChatModelSelection(config, { providerId: "backup", model: "model-c" })).toMatchObject({
      providerId: "backup",
      model: "model-c"
    });
  });

  it("falls back to the active provider default model", () => {
    expect(resolveChatModelSelection(config, null)).toMatchObject({ providerId: "primary", model: "model-b" });
  });

  it("hides models from disabled providers", () => {
    // 停用的供应商仍能被选中就等于允许发请求，开关形同虚设
    const withDisabled: AppConfig = {
      ...config,
      providers: [
        { ...config.providers[0], enabled: false },
        config.providers[1]
      ]
    };

    expect(buildChatModelChoices(withDisabled).map((choice) => choice.model)).toEqual(["model-c"]);
  });

  it("groups models by provider for the second-level picker", () => {
    const groups = groupChatModelChoices(buildChatModelChoices(config));
    expect(groups.map((group) => group.providerId)).toEqual(["primary", "backup"]);
    expect(groups[0].models.map((choice) => choice.model)).toEqual(["model-a", "model-b"]);
    expect(groups[1].models.map((choice) => choice.model)).toEqual(["model-c"]);
  });

  it("strips routing prefixes for the composer label only", () => {
    expect(shortModelName("clinepass/cline-pass/deepseek-v4-flash")).toBe("deepseek-v4-flash");
    expect(shortModelName("gpt-5")).toBe("gpt-5");
    expect(shortModelName("vendor/")).toBe("vendor");
  });

  it("moves the selection off a disabled provider", () => {
    const withDisabled: AppConfig = {
      ...config,
      providers: [
        { ...config.providers[0], enabled: false },
        config.providers[1]
      ]
    };

    expect(resolveChatModelSelection(withDisabled, { providerId: "primary", model: "model-a" }))
      .toMatchObject({ providerId: "backup", model: "model-c" });
  });
});
