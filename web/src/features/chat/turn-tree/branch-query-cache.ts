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
 * @param options 是否在后台刷新辅助数据；重发只需等待新分支时间线
 * @returns 必需数据读取完成后的 Promise，时间线读取失败时抛出异常
 */
export async function refreshSessionBranchQueries(
  queryClient: QueryClient,
  sessionId?: string,
  options: { backgroundMetadata?: boolean } = {}
): Promise<void> {
  if (!sessionId) return;
  await cancelSessionBranchQueries(queryClient, sessionId);
  // 1. 【会话分支】【缓存刷新】时间线决定消息归属，必须完成后才能展示新运行
  const timeline = queryClient.invalidateQueries({
    queryKey: ["timeline", sessionId],
    refetchType: "active"
  }, { throwOnError: true });
  // 2. 【会话分支】【缓存刷新】辅助列表照常刷新，重发无需等待这些请求
  const metadata = Promise.all([
    queryClient.invalidateQueries({
      queryKey: ["session-turn-tree", sessionId],
      refetchType: "active"
    }),
    queryClient.invalidateQueries({ queryKey: ["sessions"] }),
    queryClient.invalidateQueries({ queryKey: ["session-tree"] })
  ]);
  await Promise.all([timeline, options.backgroundMetadata ? undefined : metadata]);
}
