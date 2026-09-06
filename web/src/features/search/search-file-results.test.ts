import { describe, expect, it } from "vitest";
import type { FileNode } from "../../api/contracts";
import { searchFileResults } from "./search-file-results";

/**
 * 创建用于文件搜索行为验证的文件节点。
 * @param path 工作区相对路径
 * @returns 不包含子节点的文件
 */
function file(path: string): FileNode {
  return { name: path.split("/").at(-1)!, path, kind: "file", children: [] };
}

describe("项目文件搜索", () => {
  it("展开目录并按文件名完整匹配、前缀、子串和路径依次排序", () => {
    const nodes: FileNode[] = [{
      name: "src", path: "src", kind: "directory", children: [
        file("src/header/actions.ts"), file("src/page-header.tsx"),
        file("src/header.tsx"), file("src/header")
      ]
    }];
    expect(searchFileResults(nodes, " HEADER ").map((node) => node.path)).toEqual([
      "src/header", "src/header.tsx", "src/page-header.tsx", "src/header/actions.ts"
    ]);
  });

  it("只返回实际文件，去除重复路径并遵守结果数量限制", () => {
    const nodes = [file("README.md"), file("README.md"), file("Cargo.toml"), file("src/main.rs")];
    expect(searchFileResults(nodes, "", 2).map((node) => node.path)).toEqual(["Cargo.toml", "README.md"]);
    expect(searchFileResults(nodes, "missing")).toEqual([]);
    expect(searchFileResults(nodes, "", 0)).toEqual([]);
    expect(searchFileResults(nodes, "", -1)).toEqual([]);
  });

  it("同名文件优先展示项目根目录，再展示更深层目录", () => {
    const nodes = [file(".codebuddy/worktrees/review/README.md"), file("docs/README.md"), file("README.md")];
    expect(searchFileResults(nodes, "README").map((node) => node.path)).toEqual([
      "README.md", "docs/README.md", ".codebuddy/worktrees/review/README.md"
    ]);
  });
});
