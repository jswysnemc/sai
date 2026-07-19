import { useQuery } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import { api } from "../../../api/client";
import {
  createFileComparisonTarget,
  selectFileComparisonBase,
  type FileComparisonBases,
  type FileComparisonTarget
} from "./file-comparison-state";

/**
 * 管理多仓库文件比较基准和当前 Diff 请求。
 *
 * @returns 比较状态、查询结果与操作方法
 */
export function useFileComparison() {
  const [bases, setBases] = useState<FileComparisonBases>({});
  const [target, setTarget] = useState<FileComparisonTarget | null>(null);
  const comparison = useQuery({
    queryKey: ["git-file-diff", target?.repoRoot, target?.basePath, target?.headPath],
    queryFn: () => api.workspace.gitFileDiff(
      target!.basePath,
      target!.headPath,
      target!.repoRoot
    ),
    enabled: Boolean(target)
  });

  /**
   * 保存指定仓库的文件比较基准。
   *
   * @param repoRoot 仓库根目录
   * @param path 基准文件相对路径
   * @returns 无返回值
   */
  const selectBase = useCallback((repoRoot: string, path: string) => {
    setBases((current) => selectFileComparisonBase(current, repoRoot, path));
    setTarget((current) => current?.repoRoot === repoRoot ? null : current);
  }, []);

  /**
   * 使用指定仓库已经保存的基准创建文件比较。
   *
   * @param repoRoot 仓库根目录
   * @param path 目标文件相对路径
   * @returns 无返回值
   */
  const compare = useCallback((repoRoot: string, path: string) => {
    const nextTarget = createFileComparisonTarget(bases, repoRoot, path);
    if (nextTarget) setTarget(nextTarget);
  }, [bases]);

  /**
   * 关闭当前文件比较并恢复常规 Source Control Diff。
   *
   * @returns 无返回值
   */
  const clear = useCallback(() => setTarget(null), []);

  return {
    bases,
    target,
    data: comparison.data,
    loading: comparison.isLoading,
    error: comparison.error,
    selectBase,
    compare,
    clear
  };
}
