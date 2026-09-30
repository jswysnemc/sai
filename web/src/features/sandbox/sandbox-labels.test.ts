import { describe, expect, it } from "vitest";
import { sandboxDenialOf, sandboxScopeText } from "./sandbox-labels";

/** 固定返回中文的双语文本函数。 */
const zh = (_en: string, text: string) => text;

describe("sandboxScopeText", () => {
  it("emphasizes escalations and unavailable backends only", () => {
    expect(sandboxScopeText({ kind: "sandboxed", backend: "bwrap", network: false }, zh)).toEqual({ text: "在沙箱内执行 bwrap · 断开网络", emphasis: false });
    expect(sandboxScopeText({ kind: "unsandboxed", backend: "none", network: false }, zh).emphasis).toBe(false);
    expect(sandboxScopeText({ kind: "unavailable", backend: "bwrap", network: false }, zh).emphasis).toBe(true);
    const escalated = sandboxScopeText({ kind: "escalated", backend: "seatbelt", network: false, reasons: ["package_manager", "outside_path"] }, zh);
    expect(escalated.emphasis).toBe(true);
    expect(escalated.text).toContain("原因：包管理器、工作区外路径");
  });
});

describe("sandboxDenialOf", () => {
  it("accepts only known denial kinds", () => {
    expect(sandboxDenialOf({ sandbox_denial: { kind: "filesystem", hint: "h" } })).toEqual({ kind: "filesystem", evidence: undefined, hint: "h" });
    expect(sandboxDenialOf({ sandbox_denial: { kind: "other" } })).toBeNull();
    expect(sandboxDenialOf({ success: false })).toBeNull();
    expect(sandboxDenialOf(null)).toBeNull();
  });
});
