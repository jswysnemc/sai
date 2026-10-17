import { describe, expect, it } from "vitest";
import { isHtmlFile } from "./html-file";

describe("html file paths", () => {
  it("accepts html and htm names and rejects other files", () => {
    expect(isHtmlFile("docs/Index.HTML")).toBe(true);
    expect(isHtmlFile("C:\\pages\\home.htm")).toBe(true);
    expect(isHtmlFile("docs/readme.md")).toBe(false);
    expect(isHtmlFile("docs/page.html.txt")).toBe(false);
    expect(isHtmlFile("docs/html/readme")).toBe(false);
  });
});
