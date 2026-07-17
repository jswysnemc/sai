import { describe, expect, it } from "vitest";
import type { FileNode } from "../../api/contracts";
import { breadcrumbDirectoryPath, buildBreadcrumbParts } from "./editor-breadcrumb-utils";

const tree: FileNode[] = [{
  name: "src",
  path: "src",
  kind: "directory",
  children: [{ name: "main.ts", path: "src/main.ts", kind: "file", children: [] }]
}];

describe("editor breadcrumb utils", () => {
  it("从工作空间名称开始构建面包屑", () => {
    expect(buildBreadcrumbParts("src/main.ts", tree, "project")).toEqual([
      { label: "project", path: "", kind: "root" },
      { label: "src", path: "src", kind: "directory" },
      { label: "main.ts", path: "src/main.ts", kind: "file" }
    ]);
  });

  it("文件面包屑查询父目录，目录面包屑查询自身", () => {
    expect(breadcrumbDirectoryPath({ label: "main.ts", path: "src/main.ts", kind: "file" })).toBe("src");
    expect(breadcrumbDirectoryPath({ label: "src", path: "src", kind: "directory" })).toBe("src");
    expect(breadcrumbDirectoryPath({ label: "project", path: "", kind: "root" })).toBe("");
  });
});
