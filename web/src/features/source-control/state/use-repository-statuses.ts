import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";
import { api } from "../../../api/client";
import type { GitRepositoryState, GitRepositoryStatusesResponse } from "../../../api/contracts";

/**
 * 批量读取可见仓库状态，并支持用操作响应更新对应缓存项。
 *
 * @param repoRoots 当前需要显示的仓库根目录
 * @param enabled 是否执行查询
 * @returns 批量状态查询和单仓库缓存更新方法
 */
export function useRepositoryStatuses(repoRoots: string[], enabled: boolean) {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: ["git-statuses", repoRoots],
    queryFn: () => api.workspace.gitStatuses(repoRoots),
    enabled: enabled && repoRoots.length > 0
  });

  /**
   * 将 Git 操作响应中的仓库状态写回全部匹配的批量查询。
   *
   * @param state 最新仓库状态
   * @returns 无返回值
   */
  const updateRepositoryStatus = useCallback((state: GitRepositoryState) => {
    queryClient.setQueriesData<GitRepositoryStatusesResponse>(
      { queryKey: ["git-statuses"] },
      (current) => current ? {
        ...current,
        repositories: current.repositories.map((repository) =>
          repository.repo_root === state.repo_root ? state : repository
        )
      } : current
    );
  }, [queryClient]);

  return { ...query, updateRepositoryStatus };
}
