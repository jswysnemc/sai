import { describe, expect, it } from "vitest";
import type { FileNode } from "../../api/contracts";
import { applyLazyChildren, ancestorPaths, filterFileNodes, findFileNode, flattenVisibleNodes, parentFilePath, truncatedDirectoryFor, visibleRowRange, withAncestors } from "./file-tree-utils";

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

  it("按文件名筛选时不把父目录下的其它文件算作命中", () => {
    expect(filterFileNodes(tree, "src")).toEqual([
      { name: "src", path: "src", kind: "directory", children: [] }
    ]);
  });

  it("关键词包含斜杠时按路径匹配", () => {
    expect(filterFileNodes(tree, "src/app")).toEqual([
      {
        name: "src",
        path: "src",
        kind: "directory",
        children: [{ name: "app.tsx", path: "src/app.tsx", kind: "file", children: [] }]
      }
    ]);
  });

  it("只扁平化已展开目录", () => {
    expect(flattenVisibleNodes(tree, new Set()).map((row) => row.node.path)).toEqual(["src", "README.md"]);
    expect(flattenVisibleNodes(tree, new Set(["src"])).map((row) => [row.node.path, row.depth])).toEqual([
      ["src", 0],
      ["src/app.tsx", 1],
      ["src/theme.css", 1],
      ["README.md", 0]
    ]);
  });

  it("定位尚未加载子节点的目录", () => {
    const truncated: FileNode[] = [
      { name: "web", path: "web", kind: "directory", children: [
        { name: "src", path: "web/src", kind: "directory", children: [] }
      ] }
    ];
    expect(truncatedDirectoryFor(truncated, "web/src/app.tsx")).toBe("web/src");
    expect(truncatedDirectoryFor(tree, "src/app.tsx")).toBeNull();
    expect(ancestorPaths("web/src/app.tsx")).toEqual(["web", "web/src"]);
  });

  it("用懒加载结果补上空目录", () => {
    const nodes: FileNode[] = [{ name: "web", path: "web", kind: "directory", children: [] }];
    const lazy = new Map<string, FileNode[]>([[
      "web",
      [{ name: "index.ts", path: "web/index.ts", kind: "file", children: [] }]
    ]]);
    expect(applyLazyChildren(nodes, lazy)[0]?.children.map((node) => node.path)).toEqual(["web/index.ts"]);
    expect(applyLazyChildren(nodes, new Map())).toBe(nodes);
  });

  it("展开祖先后保持原集合", () => {
    const open = new Set(["src"]);
    expect(withAncestors(open, "src/app.tsx")).toBe(open);
    expect([...withAncestors(new Set(), "src/app.tsx")]).toEqual(["src"]);
  });

  it("按视口截取可见行", () => {
    expect(visibleRowRange(100, 0, 280, 28)).toEqual({ start: 0, end: 18 });
    expect(visibleRowRange(100, 280, 280, 28)).toEqual({ start: 2, end: 28 });
    expect(visibleRowRange(0, 0, 280, 28)).toEqual({ start: 0, end: 0 });
  });
});
