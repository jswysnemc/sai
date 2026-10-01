import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import { rebaseProviderSources, renameProviderReferences } from "./provider-identity";

describe("供应商关联改名", () => {
  it("保存期间再次改名时按最新保存的 ID 恢复密钥，保留当前编辑", () => {
    const submitted = { providers: [{ id: "saved-name", original_id: "original", display_name: "Submitted" }] } as AppConfig;
    const saved = { providers: [{ id: "saved-name", original_id: "saved-name" }] } as AppConfig;
    const draft = { providers: [{ id: "latest-name", original_id: "original", display_name: "Still editing" }] } as AppConfig;
    const next = rebaseProviderSources(draft, submitted, saved);
    expect(next.providers[0]).toEqual({ id: "latest-name", original_id: "saved-name", display_name: "Still editing" });
  });
  it("同步主会话、专用模型和智能体引用，保留自定义文本与原始 ID", () => {
    const config = {
      active_provider: "old",
      providers: [{ id: "new", original_id: "old", display_name: "old", extra_headers: { provider_id: "old" } }],
      session: { new_session_provider_id: "old", auto_title_provider_id: "other", new_session_model: "old" },
      agents: [{ id: "one", provider_id: "old", system_prompt: "old" }],
      subagent: { provider_id: "old", profiles: [{ provider_id: "old" }], model_overrides: { reviewer: { provider_id: "old" } } },
      context: { compaction_provider_id: "old" },
      memory: { extraction_provider_id: "old" },
      permission: { auto_audit_provider_id: "old" },
      git: { auto_commit_message_provider_id: "old" },
      plugins: { vision: { vision_provider_id: "old" }, custom: { provider_id: "old" } }
    } as unknown as AppConfig;
    const next = renameProviderReferences(config, "old", "new");
    expect(next.active_provider).toBe("new");
    expect(next.providers).toEqual(config.providers);
    expect(next.session).toEqual({ new_session_provider_id: "new", auto_title_provider_id: "other", new_session_model: "old" });
    expect(next.agents?.[0]).toEqual({ id: "one", provider_id: "new", system_prompt: "old" });
    expect(next.subagent?.provider_id).toBe("new");
    expect(next.subagent?.profiles?.[0].provider_id).toBe("new");
    expect(next.subagent?.model_overrides?.reviewer.provider_id).toBe("new");
    expect(next.context?.compaction_provider_id).toBe("new");
    expect(next.memory?.extraction_provider_id).toBe("new");
    expect(next.permission?.auto_audit_provider_id).toBe("new");
    expect(next.git?.auto_commit_message_provider_id).toBe("new");
    expect(next.plugins?.vision.vision_provider_id).toBe("new");
    expect(next.plugins?.custom.provider_id).toBe("old");
    expect(config.active_provider).toBe("old");
  });
});
