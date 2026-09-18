import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef } from "react";
import { api } from "../../../api/client";
import { cancelSessionBranchQueries, refreshSessionBranchQueries } from "./branch-query-cache";
import { createSessionRunScope } from "../session-run-scope";
import type { BranchSwitchResult } from "../../../api/turn-tree-contracts";

type TurnTreeOptions = {
  /** 分支指针变更成功后的本地状态清理回调 */
  onBranchChanged?: () => void;
  /** 分支切换失败后的统一错误回调 */
  onError?: (error: unknown) => void;
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
  const scopes = useRef(createSessionRunScope()).current;
  const scope = scopes.select(undefined, sessionId);
  const pendingRef = useRef<typeof scope | null>(null);
  type BranchRequest = { sessionId: string; turnId: string; scope: typeof scope };

  const tree = useQuery({
    queryKey: ["session-turn-tree", sessionId],
    queryFn: () => api.sessions.turnTree(sessionId ?? ""),
    enabled: Boolean(sessionId),
    // 1. 【会话分支】【深层历史】避免查询缓存递归比较完整分支
    structuralSharing: false,
    staleTime: 5_000
  });

  /**
   * 【会话同步】【分支请求】把会话归属写入请求变量，避免等待期间被下一次渲染替换。
   * @param action 分支接口
   * @returns 固定请求归属的变更配置
   */
  const mutationOptions = (action: (id: string, turnId: string) => Promise<BranchSwitchResult>) => ({
    mutationFn: (request: BranchRequest) => action(request.sessionId, request.turnId),
    onMutate: (request: BranchRequest) => cancelSessionBranchQueries(queryClient, request.sessionId),
    onSuccess: (_result: BranchSwitchResult, request: BranchRequest) => {
      if (scopes.isCurrent(request.scope)) options.onBranchChanged?.();
    },
    onError: (error: Error, request: BranchRequest) => {
      if (scopes.isCurrent(request.scope)) options.onError?.(error);
    },
    onSettled: async (_result: BranchSwitchResult | undefined, _error: Error | null, request: BranchRequest) => {
      try {
        await refreshSessionBranchQueries(queryClient, request.sessionId);
      } finally {
        if (pendingRef.current === request.scope) pendingRef.current = null;
      }
    }
  });
  const switchMutation = useMutation(mutationOptions(api.sessions.switchBranch));
  const undoMutation = useMutation(mutationOptions(api.sessions.undoToParent));

  /**
   * 绑定当前选择与同步操作守卫。
   * @param mutation 实际分支变更对象
   * @returns 接收轮次标识的公开操作
   */
  const bindMutation = (mutation: typeof switchMutation) => ({
    ...mutation,
    isPending: mutation.isPending && mutation.variables?.scope === scope,
    mutate: (turnId: string) => {
      if (!sessionId || !scopes.isCurrent(scope) || pendingRef.current === scope) return;
      pendingRef.current = scope;
      mutation.mutate({ sessionId, turnId, scope });
    },
    mutateAsync: (turnId: string) => {
      if (!sessionId || !scopes.isCurrent(scope) || pendingRef.current === scope) return Promise.resolve(undefined);
      pendingRef.current = scope;
      return mutation.mutateAsync({ sessionId, turnId, scope });
    }
  });

  return { tree, switchBranch: bindMutation(switchMutation), undoToParent: bindMutation(undoMutation) };
}
