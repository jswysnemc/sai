import { describe, expect, it } from "vitest";
import type { FileNode } from "../../api/contracts";
import { filterFileNodes, findFileNode, parentFilePath } from "./file-tree-utils";

const tree: FileNode[] = [
  {
    name: "src",
    path: "src",
    kind: "directory",
    children: [
      { name: "app.tsx", path: "src/app.tsx", kind: "file", children: [] },
      { name: "theme.css", path: "src/theme.css", kind: "file", children: [] }
    ]
  },
  { name: "README.md", path: "README.md", kind: "file", children: [] }
];

describe("file-tree-utils", () => {
  it("查找嵌套文件节点", () => {
    expect(findFileNode(tree, "src/app.tsx")?.name).toBe("app.tsx");
    expect(findFileNode(tree, "missing")).toBeNull();
  });

  it("过滤时保留命中文件的父目录", () => {
    expect(filterFileNodes(tree, "theme")).toEqual([
      {
        name: "src",
        path: "src",
        kind: "directory",
        children: [{ name: "theme.css", path: "src/theme.css", kind: "file", children: [] }]
      }
    ]);
  });

  it("返回文件父目录", () => {
    expect(parentFilePath("src/app.tsx")).toBe("src");
    expect(parentFilePath("README.md")).toBe("");
  });
});
