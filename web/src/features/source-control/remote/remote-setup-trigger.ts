import type { GitOperationAction } from "../../../api/git-contracts";

/**
 * 判定 Git 操作失败是否源于远端缺失，并据此引导用户补配远端。
 *
 * 后端在缺少远端时给出固定英文文案（git_operations.rs 中以 bail! 抛出），
 * 这里按这些文案识别，避免把网络失败、鉴权失败误判成未配置远端。
 */

/** 需要远端才能完成、失败后值得引导配置的操作 */
export const REMOTE_DEPENDENT_ACTIONS = ["fetch", "pull", "pull_rebase", "push", "sync"] as const;

export type RemoteDependentAction = (typeof REMOTE_DEPENDENT_ACTIONS)[number];

// 后端缺少远端时的固定文案片段，全部为小写以便忽略大小写比对
const MISSING_REMOTE_PATTERNS = [
  "repository has no remote configured",
  "origin remote is unavailable",
  "remote does not exist"
];

/**
 * 判断操作是否依赖远端。
 *
 * @param action 操作标识
 * @returns 属于远端相关操作时返回 true
 */
export function isRemoteDependentAction(action: GitOperationAction | string): action is RemoteDependentAction {
  return (REMOTE_DEPENDENT_ACTIONS as readonly string[]).includes(action);
}

/**
 * 判断错误文案是否表示远端尚未配置。
 *
 * @param message 后端返回的错误文本
 * @returns 命中缺失远端文案时返回 true
 */
export function isMissingRemoteError(message: string): boolean {
  const lower = message.toLowerCase();
  return MISSING_REMOTE_PATTERNS.some((pattern) => lower.includes(pattern));
}

/**
 * 判断一次失败的操作是否应弹出远端配置引导。
 *
 * @param action 触发失败的操作标识
 * @param message 后端返回的错误文本
 * @returns 同时满足操作依赖远端且错误为远端缺失时返回 true
 */
export function shouldPromptRemoteSetup(action: GitOperationAction | string, message: string): boolean {
  return isRemoteDependentAction(action) && isMissingRemoteError(message);
}
