import type { FileNode } from "../../api/contracts";
import { findFileNode, parentFilePath } from "./file-tree-utils";

export type BreadcrumbPart = {
  label: string;
  path: string;
  kind: "root" | "directory" | "file";
};

/**
 * 根据工作空间相对路径构建面包屑段。
 *
 * @param relativePath 当前文件相对路径
 * @param nodes 工作空间文件树
 * @param workspaceName 当前工作空间名称
 * @returns 从工作空间名称开始的面包屑路径段
 */
export function buildBreadcrumbParts(relativePath: string, nodes: FileNode[], workspaceName: string): BreadcrumbPart[] {
  const segments = relativePath.split("/").filter(Boolean);
  const parts: BreadcrumbPart[] = [{ label: workspaceName, path: "", kind: "root" }];
  let current = "";
  segments.forEach((segment, index) => {
    current = current ? `${current}/${segment}` : segment;
    const node = findFileNode(nodes, current);
    const last = index === segments.length - 1;
    parts.push({ label: segment, path: current, kind: last && node?.kind !== "directory" ? "file" : "directory" });
  });
  return parts;
}

/**
 * 计算展开面包屑时需要查询的目录。
 *
 * @param part 当前展开的面包屑段
 * @returns 工作空间相对目录，根目录返回空字符串
 */
export function breadcrumbDirectoryPath(part: BreadcrumbPart | null): string | null {
  if (!part) return null;
  if (part.kind === "root") return "";
  if (part.kind === "directory") return part.path;
  return parentFilePath(part.path);
}
