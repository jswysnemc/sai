import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { api } from "../../api/client";
import type { GitStatusEntry } from "../../api/contracts";
import { workspaceRelativePath } from "./workspace-path-utils";

export type FileTreeGitEntry = {
  entry: GitStatusEntry;
  repoRoot: string;
};

/**
 * 提供按工作区相对路径索引的多仓库 Git 状态。
 *
 * 文件树、编辑器标签徽标和编辑器行装饰共享同一份查询：
 * 查询键一致，多处调用只会命中同一缓存，不产生额外请求。
 *
 * @returns 状态映射与底层查询
 */
export function useWorkspaceGitEntries() {
  const repositories = useQuery({
    queryKey: ["git-repositories"],
    queryFn: api.workspace.gitRepositories,
    retry: false,
    staleTime: 5_000
  });
  const roots = useMemo(
    () => repositories.data?.repositories.map((repository) => repository.root) ?? [],
    [repositories.data?.repositories]
  );
  const statuses = useQuery({
    queryKey: ["file-tree-git-statuses", roots],
    queryFn: () => api.workspace.gitStatuses(roots),
    enabled: roots.length > 0,
    retry: false,
    refetchInterval: 5_000
  });
  const entries = useMemo(() => {
    const result = new Map<string, FileTreeGitEntry>();
    const workspaceRoot = repositories.data?.workspace_root ?? "";
    for (const repository of statuses.data?.repositories ?? []) {
      for (const entry of repository.entries) {
        const absolute = `${repository.repo_root.replace(/[\\/]$/, "")}/${entry.path.replace(/^[\\/]/, "")}`;
        result.set(workspaceRelativePath(absolute, workspaceRoot), {
          entry,
          repoRoot: repository.repo_root
        });
      }
    }
    return result;
  }, [repositories.data?.workspace_root, statuses.data?.repositories]);

  return { entries, repositories, statuses };
}
