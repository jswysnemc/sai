import { describe, expect, it } from "vitest";
import { attachmentLimitViolation } from "./attachment-limits";

describe("attachmentLimitViolation", () => {
  it("rejects a selection that exceeds the attachment count", () => {
    expect(attachmentLimitViolation(3, [{ size: 1 }, { size: 1 }])).toBe("too_many");
  });

  it("does not enforce a SAI-side per-file byte limit", () => {
    expect(attachmentLimitViolation(0, [{ size: 80 * 1024 * 1024 }])).toBeNull();
  });

  it("accepts four images", () => {
    expect(attachmentLimitViolation(0, Array.from({ length: 4 }, () => ({ size: 1 })))).toBeNull();
  });
});
