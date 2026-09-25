import { describe, expect, it } from "vitest";
import { absoluteWorkspacePath, pasteTargetPath, uniqueCopyPath } from "./file-tree-clipboard";

describe("file-tree-clipboard", () => {
  it("把文件粘贴到目标目录并保留原名", () => {
    expect(pasteTargetPath("src", "web/app.tsx")).toBe("src/app.tsx");
    expect(pasteTargetPath("", "README.md")).toBe("README.md");
  });

  it("用工作区根拼绝对路径", () => {
    expect(absoluteWorkspacePath("/home/sai", "src/main.rs")).toBe("/home/sai/src/main.rs");
    expect(absoluteWorkspacePath("C:\\repo", "src/main.rs")).toBe("C:\\repo\\src\\main.rs");
    expect(absoluteWorkspacePath("", "src/main.rs")).toBe("src/main.rs");
  });

  it("复制到已有路径时追加 copy", () => {
    const taken = (candidate: string) => candidate === "src/app.tsx" || candidate === "src/app copy.tsx";
    expect(uniqueCopyPath("src/app.tsx", taken)).toBe("src/app copy 2.tsx");
  });
});
