import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { api } from "../../../api/client";
import { useRepositoryStatuses } from "./use-repository-statuses";

/**
 * 聚合仓库发现、仓库选择与状态查询。
 *
 * 原本这段逻辑散在面板组件里，与视图渲染纠缠在一起；抽出后仓库切换的副作用
 * 只在一个地方发生，视图层拿到的是已经归一化的状态。
 *
 * @param enabled 是否启用查询
 * @returns 仓库列表、选中仓库与对应的状态数据
 */
export function useGitWorkspace() {
  const [selectedRoot, setSelectedRoot] = useState<string | null>(null);
  const [closedRoots, setClosedRoots] = useState<string[]>([]);

  const repositories = useQuery({
    queryKey: ["git-repositories"],
    queryFn: api.workspace.gitRepositories,
    staleTime: 5_000,
  });

  const visibleRepositories = useMemo(
    () =>
      repositories.data
        ? {
            ...repositories.data,
            repositories: repositories.data.repositories.filter(
              (repository) => !closedRoots.includes(repository.root)
            ),
          }
        : undefined,
    [closedRoots, repositories.data]
  );

  const hasRepositories = (repositories.data?.repositories.length ?? 0) > 0;

  const statusRoots = useMemo(() => {
    const roots = (visibleRepositories?.repositories ?? []).map((repository) => repository.root);
    if (selectedRoot && !roots.includes(selectedRoot)) roots.push(selectedRoot);
    return roots;
  }, [selectedRoot, visibleRepositories]);

  const repositoryStatuses = useRepositoryStatuses(
    statusRoots,
    repositories.isSuccess && hasRepositories
  );

  // 无仓库时退回单仓库状态接口，保证首次初始化场景仍能展示
  const singleStatus = useQuery({
    queryKey: ["git-status", selectedRoot],
    queryFn: () => api.workspace.gitStatus(selectedRoot ?? undefined),
    enabled: repositories.isSuccess && !hasRepositories,
  });

  useEffect(() => {
    const available = (visibleRepositories?.repositories ?? []).flatMap((repository) => [
      repository.root,
      ...repository.worktrees.map((worktree) => worktree.path),
    ]);
    setSelectedRoot((current) => (current && available.includes(current) ? current : available[0] ?? null));
  }, [visibleRepositories]);

  const state = hasRepositories
    ? repositoryStatuses.data?.repositories.find((repository) => repository.repo_root === selectedRoot)
    : singleStatus.data;

  /**
   * 关闭一个仓库分组。
   *
   * @param root 仓库根路径
   * @returns 无
   */
  const closeRepository = (root: string) =>
    setClosedRoots((current) => (current.includes(root) ? current : [...current, root]));

  return {
    repositories,
    visibleRepositories,
    hasRepositories,
    repositoryStatuses,
    selectedRoot,
    setSelectedRoot,
    closedRoots,
    closeRepository,
    showAllRepositories: () => setClosedRoots([]),
    state,
    statusLoading: hasRepositories ? repositoryStatuses.isLoading : singleStatus.isLoading,
    statusError: hasRepositories ? repositoryStatuses.error : singleStatus.error,
    refetchStatus: hasRepositories ? repositoryStatuses.refetch : singleStatus.refetch,
    /** 当前视图涉及的全部仓库状态，单仓库场景回落到唯一状态 */
    allStates: hasRepositories ? repositoryStatuses.data?.repositories ?? [] : state ? [state] : [],
  };
}
