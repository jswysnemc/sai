import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import { clampNumber, patchJevAudit, patchJevMemoryInjection, patchJevRouting, readJevConfig, readJevMemoryInjection, releaseJevEndpoint, selectJevEndpoint } from "./jev-config";

const base = { active_provider: "p", providers: [] } as unknown as AppConfig;

describe("jev config helpers", () => {
  it("fills defaults for missing sections", () => {
    const jev = readJevConfig({ ...base, jev: { routing: { enabled: true } } } as unknown as AppConfig);
    expect(jev.routing.enabled).toBe(true);
    expect(jev.routing.max_tools).toBe(6);
    expect(jev.audit.minimum_probability).toBe(0.9);
    expect(jev.endpoint_id).toBe("");
    expect(jev.routing.prompt_segments).toBe(true);
  });

  it("writes memory injection to plugins.memory and keeps an existing root mirror in sync", () => {
    const plain = patchJevMemoryInjection({ ...base, plugins: { memory: { enabled: false } } } as unknown as AppConfig, true);
    expect(plain.plugins?.memory).toEqual({ enabled: false, jev_injection: true });
    expect(plain.memory).toBeUndefined();
    expect(readJevMemoryInjection(plain)).toBe(true);
    const mirrored = patchJevMemoryInjection({ ...plain, memory: { jev_injection: true } } as unknown as AppConfig, false);
    expect(readJevMemoryInjection(mirrored)).toBe(false);
    expect((mirrored.plugins?.memory as { jev_injection: boolean }).jev_injection).toBe(false);
  });

  it("patches routing and audit independently", () => {
    const routed = patchJevRouting(base, { enabled: true });
    const audited = patchJevAudit(routed, { enabled: true, minimum_confidence: 0.85 });
    expect(audited.jev?.routing.enabled).toBe(true);
    expect(audited.jev?.audit.enabled).toBe(true);
    expect(audited.jev?.audit.minimum_probability).toBe(0.9);
  });

  it("clears the reference when the selected endpoint is removed", () => {
    const selected = selectJevEndpoint(base, "jev-1");
    expect(releaseJevEndpoint(selected, "jev-2").jev?.endpoint_id).toBe("jev-1");
    expect(releaseJevEndpoint(selected, "jev-1").jev?.endpoint_id).toBe("");
  });

  it("clamps numeric input", () => {
    expect(clampNumber("1.4", 0, 1)).toBe(1);
    expect(clampNumber("", 1, 60, true)).toBe(1);
    expect(clampNumber("7.6", 1, 60, true)).toBe(8);
    expect(clampNumber("abc", 0.5, 1)).toBe(0.5);
  });
});
