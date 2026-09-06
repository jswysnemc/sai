import { describe, expect, it } from "vitest";
import { isAbsoluteFilePath, workspaceRelativePath } from "./workspace-path-utils";

describe("workspace path utils", () => {
  it("把工作空间内绝对路径转换为相对路径", () => {
    expect(workspaceRelativePath("/home/user/project/src/main.ts", "/home/user/project")).toBe("src/main.ts");
    expect(workspaceRelativePath("src/main.ts", "/home/user/project")).toBe("src/main.ts");
  });

  it("不会截断名称相似但不属于工作空间的路径", () => {
    expect(workspaceRelativePath("/home/user/project-copy/main.ts", "/home/user/project")).toBe("/home/user/project-copy/main.ts");
  });

  it("清理 Windows 扩展路径前缀", () => {
    expect(workspaceRelativePath("\\\\?\\C:\\Users\\xz\\demo\\src\\main.rs", "\\\\?\\C:\\Users\\xz\\demo")).toBe("src/main.rs");
  });

  it("外部文件保留绝对路径，不进入当前工作区面包屑", () => {
    expect(isAbsoluteFilePath(workspaceRelativePath("/home/user/notes.md", "/home/user/project"))).toBe(true);
    expect(isAbsoluteFilePath(workspaceRelativePath("/home/user/project/src/main.ts", "/home/user/project"))).toBe(false);
    expect(isAbsoluteFilePath("C:\\notes\\main.ts")).toBe(true);
    expect(isAbsoluteFilePath("../notes.md")).toBe(false);
  });
});
