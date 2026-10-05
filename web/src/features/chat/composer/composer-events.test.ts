import { describe, expect, it } from "vitest";
import { appendComposerText } from "./composer-events";

describe("appendComposerText", () => {
  it("appends selected text on a new line", () => {
    expect(appendComposerText("hello", { content: "world" })).toBe("hello\nworld");
  });

  it("quotes each line when requested", () => {
    expect(appendComposerText("", { content: "one\ntwo", quote: true })).toBe("> one\n> two");
  });
});
