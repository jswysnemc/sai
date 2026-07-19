import type { GitStatusEntry } from "../../../api/contracts";

type GitChangeTreeDirectoryRow = {
  kind: "directory";
  path: string;
  name: string;
  depth: number;
};

type GitChangeTreeFileRow = {
  kind: "file";
  entry: GitStatusEntry;
  name: string;
  depth: number;
};

export type GitChangeTreeRow = GitChangeTreeDirectoryRow | GitChangeTreeFileRow;

type MutableDirectory = {
  path: string;
  name: string;
  directories: Map<string, MutableDirectory>;
  files: Array<{ entry: GitStatusEntry; name: string }>;
};

/**
 * 将 Git 文件路径转换为可折叠的目录树行。
 *
 * @param entries 当前分区文件
 * @param collapsedPaths 已折叠目录路径
 * @returns 按目录优先、名称排序的扁平渲染行
 */
export function buildGitChangeTreeRows(
  entries: GitStatusEntry[],
  collapsedPaths: ReadonlySet<string> = new Set()
): GitChangeTreeRow[] {
  const root: MutableDirectory = {
    path: "",
    name: "",
    directories: new Map(),
    files: []
  };

  // 1. 将跨平台路径分段后写入目录树
  for (const entry of entries) {
    const segments = entry.path.split(/[\\/]+/).filter(Boolean);
    const fileName = segments.pop() ?? entry.path;
    let directory = root;
    for (const segment of segments) {
      const path = directory.path ? `${directory.path}/${segment}` : segment;
      let child = directory.directories.get(segment);
      if (!child) {
        child = { path, name: segment, directories: new Map(), files: [] };
        directory.directories.set(segment, child);
      }
      directory = child;
    }
    directory.files.push({ entry, name: fileName });
  }

  // 2. 深度优先展开未折叠目录，保持稳定排序
  const rows: GitChangeTreeRow[] = [];
  appendDirectoryRows(root, 0, collapsedPaths, rows);
  return rows;
}

/**
 * 递归追加单个目录的子目录和文件。
 *
 * @param directory 当前目录节点
 * @param depth 当前渲染深度
 * @param collapsedPaths 已折叠目录路径
 * @param rows 输出行数组
 * @returns 无返回值
 */
function appendDirectoryRows(
  directory: MutableDirectory,
  depth: number,
  collapsedPaths: ReadonlySet<string>,
  rows: GitChangeTreeRow[]
) {
  const directories = [...directory.directories.values()]
    .sort((left, right) => left.name.localeCompare(right.name));
  const files = [...directory.files]
    .sort((left, right) => left.name.localeCompare(right.name));

  for (const child of directories) {
    rows.push({ kind: "directory", path: child.path, name: child.name, depth });
    if (!collapsedPaths.has(child.path)) {
      appendDirectoryRows(child, depth + 1, collapsedPaths, rows);
    }
  }
  for (const file of files) {
    rows.push({ kind: "file", entry: file.entry, name: file.name, depth });
  }
}
