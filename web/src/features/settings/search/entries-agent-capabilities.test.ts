import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import { buildCapabilitySearchEntries } from "./entries-agent-capabilities";
import { searchEntryHref } from "./settings-search";

describe("capability field links", () => {
  it("opens the intended agent and permission tab without indexing prompt contents", () => {
    const config = { agents: [{ id: "reviewer", name: "Reviewer", system_prompt: "private prompt" }], skills: { progressive: true } } as unknown as AppConfig;
    const entries = buildCapabilitySearchEntries(config);
    const hit = entries.find((entry) => entry.item === "reviewer" && entry.anchor === "agents.enabled_tools")!;
    expect(searchEntryHref(hit)).toContain("&item=reviewer&view=tools");
    expect(JSON.stringify(entries)).not.toContain("private prompt");
    expect(entries.find((entry) => entry.anchor === "skills.progressive")?.view).toBe("behavior");
  });

  it("indexes only the selected hook action type and retains its item identifier", () => {
    const config = { hooks: { items: [{ name: "audit", kind: "http", requests: [{ url: "https://private.example" }] }] } } as unknown as AppConfig;
    const entries = buildCapabilitySearchEntries(config).filter((entry) => entry.section === "hooks");
    expect(entries.some((entry) => entry.anchor === "hooks.script")).toBe(false);
    expect(entries.find((entry) => entry.anchor === "hooks.requests")?.item).toBe("0:audit");
    expect(JSON.stringify(entries)).not.toContain("private.example");
  });
});
