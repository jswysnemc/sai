/**
 * 浏览器目录选择的解析工具。
 *
 * `<input webkitdirectory>` 出于安全考虑只交出相对路径（形如 `sandbox/src/main.rs`），
 * 拿不到用户机器上的绝对路径。所以这里把相对路径的首段目录名，
 * 与服务端已知的可浏览根目录拼接，还原出服务端可用的绝对路径。
 *
 * 这也决定了功能边界：只有当选中的目录位于某个允许根之下时才能定位。
 */

/** 从一次目录选择中解析出的候选路径。 */
export type PickedDirectory = {
  /** 用户选中的目录名，即相对路径的首段 */
  name: string;
  /** 与各允许根拼接后的候选绝对路径 */
  candidates: string[];
};

/**
 * 从浏览器目录选择的文件相对路径中取出被选目录名。
 *
 * @param relativePaths 各文件的 webkitRelativePath
 * @returns 被选目录名；无法解析时返回空串
 */
export function pickedDirectoryName(relativePaths: string[]): string {
  for (const path of relativePaths) {
    const segment = path.split("/").filter(Boolean)[0];
    if (segment) return segment;
  }
  return "";
}

/**
 * 把被选目录名拼到各允许根之下，得到候选绝对路径。
 *
 * 同名目录可能出现在多个根下，因此返回列表而不是单值，由调用方确认。
 *
 * @param name 被选目录名
 * @param roots 服务端允许浏览的根目录路径
 * @returns 候选绝对路径，按根的给出顺序排列
 */
export function resolveDirectoryCandidates(name: string, roots: string[]): string[] {
  if (!name) return [];
  return roots
    .filter(Boolean)
    .map((root) => `${root.replace(/\/+$/u, "")}/${name}`);
}

/**
 * 解析一次浏览器目录选择。
 *
 * @param relativePaths 各文件的 webkitRelativePath
 * @param roots 服务端允许浏览的根目录路径
 * @returns 目录名与候选绝对路径
 */
export function parsePickedDirectory(relativePaths: string[], roots: string[]): PickedDirectory {
  const name = pickedDirectoryName(relativePaths);
  return { name, candidates: resolveDirectoryCandidates(name, roots) };
}
