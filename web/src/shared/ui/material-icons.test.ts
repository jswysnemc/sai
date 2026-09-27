import { describe, expect, it } from "vitest";
import { fileNameForLanguage } from "./material-icons";

describe("fileNameForLanguage", () => {
  it("按语言标签选已有的类型图标文件", () => {
    expect(fileNameForLanguage("Rust")).toBe("snippet.rs");
    expect(fileNameForLanguage("typescript")).toBe("snippet.ts");
    expect(fileNameForLanguage("c++")).toBe("snippet.cpp");
    expect(fileNameForLanguage("Dockerfile")).toBe("Dockerfile");
  });

  it("空标签和未知语言仍能落到文件图标", () => {
    expect(fileNameForLanguage("")).toBe("snippet.txt");
    expect(fileNameForLanguage(undefined)).toBe("snippet.txt");
    expect(fileNameForLanguage("mermaid")).toBe("snippet.mermaid");
  });
});
