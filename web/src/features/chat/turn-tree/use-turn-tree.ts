import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../../api/client";
import { cancelSessionBranchQueries, refreshSessionBranchQueries } from "./branch-query-cache";

type TurnTreeOptions = {
  /** 分支指针变更成功后的本地状态清理回调 */
  onBranchChanged?: () => void;
};

/**
 * 读取并操作会话分支树。
 *
 * 切换分支后需要同时刷新树与时间线：前者更新高亮位置，后者重放该分支历史。
 *
 * @param sessionId 当前会话标识
 * @param options 分支变化后的本地状态回调
 * @returns 树数据与切换、撤销操作
 */
export function useTurnTree(sessionId?: string, options: TurnTreeOptions = {}) {
  const queryClient = useQueryClient();

  const tree = useQuery({
    queryKey: ["session-turn-tree", sessionId],
    queryFn: () => api.sessions.turnTree(sessionId ?? ""),
    enabled: Boolean(sessionId),
    staleTime: 5_000
  });

  const switchBranch = useMutation({
    mutationFn: (turnId: string) => api.sessions.switchBranch(sessionId ?? "", turnId),
    onMutate: () => cancelSessionBranchQueries(queryClient, sessionId),
    onSuccess: () => options.onBranchChanged?.(),
    onSettled: () => refreshSessionBranchQueries(queryClient, sessionId)
  });

  const undoToParent = useMutation({
    mutationFn: (turnId: string) => api.sessions.undoToParent(sessionId ?? "", turnId),
    onMutate: () => cancelSessionBranchQueries(queryClient, sessionId),
    onSuccess: () => options.onBranchChanged?.(),
    onSettled: () => refreshSessionBranchQueries(queryClient, sessionId)
  });

  return { tree, switchBranch, undoToParent };
}
