import { describe, expect, it } from "vitest";
import { buildOperationNotice, compactNoticeMessage } from "./operation-notice";

describe("compactNoticeMessage", () => {
  it("trims surrounding whitespace", () => {
    expect(compactNoticeMessage("  pushed to origin  ")).toBe("pushed to origin");
  });

  it("keeps messages at the length limit intact", () => {
    const message = "a".repeat(260);
    expect(compactNoticeMessage(message)).toBe(message);
  });

  it("truncates longer messages to the limit including the ellipsis", () => {
    const compacted = compactNoticeMessage("a".repeat(400));
    expect(compacted).toHaveLength(260);
    expect(compacted.endsWith("...")).toBe(true);
  });
});

describe("buildOperationNotice", () => {
  it("builds a notice from a non-empty message", () => {
    expect(buildOperationNotice(1, "success", "push", " done ")).toEqual({
      id: 1,
      kind: "success",
      action: "push",
      message: "done"
    });
  });

  it("returns null when the message is blank", () => {
    expect(buildOperationNotice(1, "error", "push", "   ")).toBeNull();
  });
});
