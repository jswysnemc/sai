import type { QueryClient } from "@tanstack/react-query";

/**
 * 取消当前会话切换前遗留的分支查询。
 *
 * @param queryClient React Query 客户端
 * @param sessionId 当前会话标识
 * @returns 取消完成后的 Promise
 */
export async function cancelSessionBranchQueries(
  queryClient: QueryClient,
  sessionId?: string
): Promise<void> {
  if (!sessionId) return;
  await Promise.all([
    queryClient.cancelQueries({ queryKey: ["session-turn-tree", sessionId] }),
    queryClient.cancelQueries({ queryKey: ["timeline", sessionId] })
  ]);
}

/**
 * 在活动叶子变化后重新读取分支树与当前分支时间线。
 *
 * @param queryClient React Query 客户端
 * @param sessionId 当前会话标识
 * @returns 新树和新时间线读取完成后的 Promise
 */
export async function refreshSessionBranchQueries(
  queryClient: QueryClient,
  sessionId?: string
): Promise<void> {
  if (!sessionId) return;
  await cancelSessionBranchQueries(queryClient, sessionId);
  await Promise.all([
    queryClient.refetchQueries({
      queryKey: ["session-turn-tree", sessionId],
      type: "active"
    }),
    queryClient.refetchQueries({
      queryKey: ["timeline", sessionId],
      type: "active"
    }),
    queryClient.invalidateQueries({ queryKey: ["sessions"] }),
    queryClient.invalidateQueries({ queryKey: ["session-tree"] })
  ]);
}
