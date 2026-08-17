import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { api } from "../../api/client";
import { buildEditorGitLines, type EditorGitLine } from "./editor-git-decorations";
import type { FileTreeGitEntry } from "./use-workspace-git-entries";

/**
 * 为当前打开的文件提供 Git 行装饰。
 *
 * 只有当文件在仓库状态里存在跟踪中的改动时才拉取单文件补丁；
 * 未跟踪文件整个都是新增，没有可标注的基线，直接返回空。
 * 查询键沿用 git-review-diff 前缀，暂存等操作后随状态作用域一起失效。
 *
 * @param path 工作区相对文件路径
 * @param entries 按工作区路径索引的 Git 状态
 * @returns 行装饰列表；无改动或不可用时为空数组
 */
export function useEditorGitDiff(
  path: string | null,
  entries: ReadonlyMap<string, FileTreeGitEntry>
): EditorGitLine[] {
  const item = path ? entries.get(path) : undefined;
  const enabled = Boolean(item && !item.entry.untracked);
  const diff = useQuery({
    queryKey: ["git-review-diff", item?.repoRoot, "working_tree", item?.entry.path],
    queryFn: () => api.workspace.gitReviewDiff("working_tree", item!.entry.path, item!.repoRoot),
    enabled,
    retry: false,
    refetchInterval: 5_000
  });

  const patch = enabled ? diff.data?.patch ?? "" : "";
  const gitLines = useMemo(() => (patch ? buildEditorGitLines(patch) : []), [patch]);

  return gitLines;
}
