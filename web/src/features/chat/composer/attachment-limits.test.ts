import { describe, expect, it } from "vitest";
import {
  attachmentLimitViolation,
  MAX_IMAGE_ATTACHMENT_BYTES
} from "./attachment-limits";

describe("attachmentLimitViolation", () => {
  it("rejects a selection that exceeds the attachment count", () => {
    expect(attachmentLimitViolation(3, [{ size: 1 }, { size: 1 }])).toBe("too_many");
  });

  it("rejects an image that exceeds the per-file byte limit", () => {
    expect(attachmentLimitViolation(0, [{ size: MAX_IMAGE_ATTACHMENT_BYTES + 1 }])).toBe("too_large");
  });

  it("accepts four images within the byte limit", () => {
    expect(attachmentLimitViolation(0, Array.from({ length: 4 }, () => ({ size: 1 })))).toBeNull();
  });
});
