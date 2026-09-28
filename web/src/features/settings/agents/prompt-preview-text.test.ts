import { describe, expect, it } from "vitest";
import { promptPreviewText } from "./prompt-preview-text";

describe("prompt preview", () => {
  it("removes mark wrappers without executing or interpreting other prompt text", () => {
    expect(promptPreviewText('<mark class="role">默认 agent</mark>\n<tool>keep literal</tool>')).toBe('默认 agent\n<tool>keep literal</tool>');
  });
});
