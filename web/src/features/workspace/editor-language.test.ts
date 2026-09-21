import { describe, expect, it } from "vitest";
import { languageForPath } from "./editor-language";

describe("editor language detection", () => {
  it("uses the diff grammar for patch files on Unix and Windows paths", () => {
    expect(languageForPath("changes.patch")).toBe("diff");
    expect(languageForPath("C:\\work\\changes.diff")).toBe("diff");
  });
});
