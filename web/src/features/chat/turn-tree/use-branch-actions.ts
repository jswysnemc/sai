import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api } from "../../../api/client";
import type { ApiError } from "../../../api/api-error";
import { cancelSessionBranchQueries, refreshSessionBranchQueries } from "./branch-query-cache";

type BranchActionsOptions = {
  /** 当前会话标识 */
  sessionId?: string;
  /** 会话是否正在运行；运行中禁止改动分支指针 */
  running: boolean;
  /** 切换成功后聚焦输入框，提示用户可以从此处继续 */
  onFocusComposer?: () => void;
  /** 分支切换成功后清除旧实时运行投影 */
  resetRun?: () => void;
  /** 动作失败时上报，由调用方决定展示方式 */
  onError?: (error: unknown, fallbackEn: string, fallbackZh: string) => void;
};

/**
 * 封装基于会话树的三个分支动作。
 *
 * 三者共用同一套语义：只移动活动叶子指针，不删除任何轮次。被移出的分支
 * 仍留在树里，可以随时切回对比。
 *
 * @param options 会话标识、运行状态与回调
 * @returns 分支动作与进行中标记
 */
export function useBranchActions({ sessionId, running, onFocusComposer, resetRun, onError }: BranchActionsOptions) {
  const queryClient = useQueryClient();
  const [pending, setPending] = useState(false);

  /**
   * 在同一次守卫与错误处理下执行一个分支动作。
   *
   * @param action 实际的异步动作
   * @param fallbackEn 失败时的英文兜底文案
   * @param fallbackZh 失败时的中文兜底文案
   * @returns 执行完成的 Promise
   */
  const runGuarded = async (action: () => Promise<void>, fallbackEn: string, fallbackZh: string) => {
    // 1. 无会话或运行中时不改动分支指针，后端同样会拒绝
    if (!sessionId || running || pending) return;
    setPending(true);
    try {
      await action();
    } catch (error) {
      onError?.(error as ApiError, fallbackEn, fallbackZh);
    } finally {
      setPending(false);
    }
  };

  /**
   * 把对话位置切到指定轮次，之后发送的消息会成为该轮次的新分支。
   *
   * 后端在插入新轮次时以当前活动叶子作为父节点，因此"切换 + 发消息"
   * 就等于在任意历史位置开分支，不需要额外接口。
   *
   * @param turnId 目标轮次
   * @returns 执行完成的 Promise
   */
  const continueFrom = (turnId: string) =>
    runGuarded(
      async () => {
        await cancelSessionBranchQueries(queryClient, sessionId);
        await api.sessions.switchBranch(sessionId ?? "", turnId);
        resetRun?.();
        await refreshSessionBranchQueries(queryClient, sessionId);
        onFocusComposer?.();
      },
      "Failed to continue from this turn",
      "从该轮次继续失败"
    );

  /**
   * 把对话位置退回指定轮次的父轮次。
   *
   * 与删除式撤销不同，被退出的轮次仍保留在树中，可随时切回。
   *
   * @param turnId 要退出的轮次
   * @returns 执行完成的 Promise
   */
  const undoToParent = (turnId: string) =>
    runGuarded(
      async () => {
        await cancelSessionBranchQueries(queryClient, sessionId);
        await api.sessions.undoToParent(sessionId ?? "", turnId);
        resetRun?.();
        await refreshSessionBranchQueries(queryClient, sessionId);
      },
      "Failed to undo to the previous turn",
      "退回上一轮失败"
    );

  /**
   * 把对话位置切到指定轮次的父轮次，供重试时重新发起使用。
   *
   * 重试因此不再删除旧回答：新回答成为兄弟分支，两者可以来回对比。
   *
   * @param turnId 要重试的轮次
   * @returns 切换是否成功
   */
  const moveToParentForRetry = async (turnId: string): Promise<boolean> => {
    if (!sessionId) return false;
    await cancelSessionBranchQueries(queryClient, sessionId);
    await api.sessions.undoToParent(sessionId, turnId);
    return true;
  };

  return { continueFrom, undoToParent, moveToParentForRetry, pending };
}
