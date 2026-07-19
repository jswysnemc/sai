export type FileComparisonTarget = {
  repoRoot: string;
  basePath: string;
  headPath: string;
};

export type FileComparisonBases = Record<string, string>;

/**
 * 保存单个仓库的文件比较基准。
 *
 * @param current 当前仓库基准映射
 * @param repoRoot 仓库根目录
 * @param path 基准文件相对路径
 * @returns 更新后的仓库基准映射
 */
export function selectFileComparisonBase(
  current: FileComparisonBases,
  repoRoot: string,
  path: string
): FileComparisonBases {
  return { ...current, [repoRoot]: path };
}

/**
 * 根据仓库基准和目标文件创建比较请求。
 *
 * @param bases 当前仓库基准映射
 * @param repoRoot 仓库根目录
 * @param headPath 目标文件相对路径
 * @returns 有效比较目标；基准缺失或路径相同时返回空值
 */
export function createFileComparisonTarget(
  bases: FileComparisonBases,
  repoRoot: string,
  headPath: string
): FileComparisonTarget | null {
  const basePath = bases[repoRoot];
  if (!basePath || basePath === headPath) return null;
  return { repoRoot, basePath, headPath };
}
