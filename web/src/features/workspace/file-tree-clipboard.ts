/** 文件树里暂存的剪切或复制。 */
export type TreeClipboard = {
  mode: "cut" | "copy";
  path: string;
  directory: boolean;
};

/**
 * 取相对路径的最后一段。
 *
 * @param path 工作区相对路径
 * @returns 文件或目录名
 */
export function fileBaseName(path: string): string {
  const index = path.lastIndexOf("/");
  return index < 0 ? path : path.slice(index + 1);
}

/**
 * 把条目粘贴到目标目录下，保留原名。
 *
 * @param directory 目标目录相对路径，根目录为空
 * @param sourcePath 被剪切或复制的相对路径
 * @returns 粘贴后的相对路径
 */
export function pasteTargetPath(directory: string, sourcePath: string): string {
  const name = fileBaseName(sourcePath);
  return directory ? `${directory}/${name}` : name;
}

/**
 * 拼出工作区绝对路径，分隔符跟随根路径。
 *
 * @param root 工作区根路径
 * @param relative 工作区相对路径
 * @returns 绝对路径；没有根时退回相对路径
 */
export function absoluteWorkspacePath(root: string, relative: string): string {
  const base = root.replace(/[\\/]+$/, "");
  if (!base) return relative;
  if (!relative) return base;
  const separator = base.includes("\\") && !base.includes("/") ? "\\" : "/";
  return `${base}${separator}${relative.split("/").join(separator)}`;
}

/**
 * 复制到已存在的路径时，在文件名后追加 copy。
 *
 * @param path 期望写入的相对路径
 * @param taken 该路径是否已被占用
 * @returns 不冲突的相对路径
 */
export function uniqueCopyPath(path: string, taken: (candidate: string) => boolean): string {
  if (!taken(path)) return path;
  const slash = path.lastIndexOf("/");
  const directory = slash < 0 ? "" : path.slice(0, slash + 1);
  const name = slash < 0 ? path : path.slice(slash + 1);
  const dot = name.lastIndexOf(".");
  const stem = dot > 0 ? name.slice(0, dot) : name;
  const extension = dot > 0 ? name.slice(dot) : "";
  for (let index = 1; index < 100; index += 1) {
    const candidate = `${directory}${stem} copy${index === 1 ? "" : ` ${index}`}${extension}`;
    if (!taken(candidate)) return candidate;
  }
  return `${directory}${stem} copy${extension}`;
}
