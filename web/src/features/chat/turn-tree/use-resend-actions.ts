import { useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { api } from "../../../api/client";
import type { RunMode, RunModelSelection, SessionTimeline, ThinkingLevel } from "../../../api/contracts";
import { retryableTurnId } from "../conversation-display";
import { refreshSessionBranchQueries } from "./branch-query-cache";
import { createSessionRunScope } from "../session-run-scope";

type ResendOptions = {
  sessionId?: string;
  /** 会话是否正在运行；运行中不允许改动分支指针 */
  running: boolean;
  mode: RunMode;
  selection?: RunModelSelection;
  thinkingLevel: ThinkingLevel;
  agentId?: string;
  /** 把活动叶子退回指定轮次的父节点 */
  moveToParent: (turnId: string) => Promise<boolean>;
  /** 清理旧的实时投影，避免同一条用户消息渲染两次 */
  resetRun: () => void;
  /** 以给定内容发起新一轮 */
  startRun: (content: string, imageUrls?: string[]) => Promise<void>;
  onError: (error: unknown, fallbackEn: string, fallbackZh: string) => void;
};

/**
 * 封装「重试」与「编辑重发」两个基于分支的重新发起动作。
 *
 * 两者共享同一套语义：先把活动叶子退回目标轮次的父节点，再以新内容发起一轮，
 * 于是新回答成为旧轮次的兄弟分支，旧轮次原样保留在树里。
 *
 * @param options 会话状态、运行参数与分支操作
 * @returns 重试与编辑重发方法、进行中状态
 */
export function useResendActions(options: ResendOptions) {
  const queryClient = useQueryClient();
  const scopes = useRef(createSessionRunScope()).current;
  const scope = scopes.select(undefined, options.sessionId);
  const [pendingScope, setPendingScope] = useState<typeof scope | null>(null);
  const pendingRef = useRef<typeof scope | null>(null);

  /**
   * 退回目标轮次的父节点后以给定内容重新发起。
   *
   * @param content 要提交的正文
   * @param imageUrls 要提交的图片；无图时传 undefined
   * @param candidateTurnId 被操作的轮次标识
   * @param requireTurn 解析不到轮次时是否中止。编辑必须中止：
   *        否则会退化成在会话末尾追加一条消息，与「从被编辑处分支」的语义完全不同
   * @returns 发起完成的 Promise
   */
  const resendFromParent = async (
    content: string,
    imageUrls: string[] | undefined,
    candidateTurnId: string | null,
    requireTurn: boolean
  ) => {
    const sessionId = options.sessionId;
    if (!sessionId || options.running || pendingRef.current === scope || !scopes.isCurrent(scope)) return;
    if (!content.trim() && !(imageUrls && imageUrls.length > 0)) return;
    // 1. 【会话重发】【操作反馈】同步拦截重复点击，并在请求前更新界面状态
    pendingRef.current = scope;
    setPendingScope(scope);
    try {
      // 2. 【会话重发】【轮次定位】已展示的持久化轮次直接使用缓存，实时轮次缺失时再读取最新时间线
      const cached = queryClient.getQueryData<SessionTimeline>(["timeline", sessionId]);
      let turnId = retryableTurnId(cached?.turns ?? [], candidateTurnId);
      if (!turnId) {
        const refreshed = await api.sessions.timeline(sessionId);
        if (!scopes.isCurrent(scope)) return;
        turnId = retryableTurnId(refreshed.turns, candidateTurnId);
      }
      // 3. 【会话重发】【轮次校验】编辑目标缺失时中止，避免追加到会话末尾
      if (!turnId && requireTurn) {
        options.onError(
          new Error("turn not found"),
          "That turn is no longer in the conversation, so it cannot be edited",
          "该轮次已不在当前对话中，无法编辑重发"
        );
        return;
      }
      // 4. 【会话重发】【分支切换】保留旧轮次，清理旧实时投影并等待新分支时间线
      if (turnId && !(await options.moveToParent(turnId))) return;
      if (!scopes.isCurrent(scope)) return;
      options.resetRun();
      await refreshSessionBranchQueries(queryClient, sessionId, { backgroundMetadata: true });
      if (!scopes.isCurrent(scope)) return;
      // 5. 【会话重发】【运行提交】复用当前模式、模型与思考等级重新提交
      await options.startRun(content, imageUrls);
    } catch (error) {
      if (scopes.isCurrent(scope)) options.onError(error, "Failed to resend the turn", "重新发送失败");
    } finally {
      if (pendingRef.current === scope) pendingRef.current = null;
      setPendingScope((current) => current === scope ? null : current);
    }
  };

  return {
    pending: pendingScope === scope,
    /**
     * 以原内容重试某一轮。
     *
     * @param content 原轮次的用户正文
     * @param imageUrls 原轮次的图片
     * @param turnId 被重试的轮次标识
     * @returns 重试完成的 Promise
     */
    retry: (content: string, imageUrls: string[] | undefined, turnId: string | null) =>
      resendFromParent(content, imageUrls, turnId, false),
    /**
     * 以改写后的内容重发某一轮。
     *
     * @param turnId 被编辑的轮次标识
     * @param content 改写后的正文
     * @param imageUrls 改写后的图片
     * @returns 重发完成的 Promise
     */
    editAndResend: (turnId: string | null, content: string, imageUrls: string[]) =>
      resendFromParent(content, imageUrls.length > 0 ? imageUrls : undefined, turnId, true)
  };
}
