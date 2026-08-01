import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../../api/client";

/**
 * 读取并操作会话分支树。
 *
 * 切换分支后需要同时刷新树与时间线：前者更新高亮位置，后者重放该分支历史。
 *
 * @param sessionId 当前会话标识
 * @returns 树数据与切换、撤销操作
 */
export function useTurnTree(sessionId?: string) {
  const queryClient = useQueryClient();

  const tree = useQuery({
    queryKey: ["session-turn-tree", sessionId],
    queryFn: () => api.sessions.turnTree(sessionId ?? ""),
    enabled: Boolean(sessionId),
    staleTime: 5_000
  });

  /** 分支变化后刷新树与会话内容。 */
  const invalidateAfterBranchChange = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["session-turn-tree", sessionId] }),
      queryClient.invalidateQueries({ queryKey: ["timeline", sessionId] }),
      queryClient.invalidateQueries({ queryKey: ["sessions"] })
    ]);
  };

  const switchBranch = useMutation({
    mutationFn: (turnId: string) => api.sessions.switchBranch(sessionId ?? "", turnId),
    onSuccess: invalidateAfterBranchChange
  });

  const undoToParent = useMutation({
    mutationFn: (turnId: string) => api.sessions.undoToParent(sessionId ?? "", turnId),
    onSuccess: invalidateAfterBranchChange
  });

  return { tree, switchBranch, undoToParent };
}
