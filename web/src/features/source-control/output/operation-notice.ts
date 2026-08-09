import type { GitOperationAction } from "../../../api/git-contracts";

/**
 * Git 操作结果提示的构造与文案裁剪。
 *
 * 操作成功或失败后短暂浮出一条提示，与底部错误行、输出面板并存：
 * 这里只负责数据构造，展示与消失时机由 git-operation-toast 组件处理。
 */
export type OperationNoticeKind = "success" | "error";

export type OperationNotice = {
  id: number;
  kind: OperationNoticeKind;
  action: GitOperationAction | string;
  message: string;
};

const MAX_NOTICE_LENGTH = 260;

/**
 * 裁剪过长的 Git 输出，避免提示框撑满屏幕。
 *
 * @param value 原始消息文本
 * @returns 去除首尾空白并限长后的文本
 */
export function compactNoticeMessage(value: string): string {
  const message = value.trim();
  if (message.length <= MAX_NOTICE_LENGTH) return message;
  return `${message.slice(0, MAX_NOTICE_LENGTH - 3)}...`;
}

/**
 * 构造一条操作结果提示。
 *
 * id 由调用方传入的序号生成，便于同一操作重复执行时重新触发进场动画。
 *
 * @param id 递增序号
 * @param kind 提示类型
 * @param action 触发提示的操作标识
 * @param message 提示正文
 * @returns 提示数据；正文为空时返回 null
 */
export function buildOperationNotice(
  id: number,
  kind: OperationNoticeKind,
  action: GitOperationAction | string,
  message: string
): OperationNotice | null {
  const compacted = compactNoticeMessage(message);
  if (!compacted) return null;
  return { id, kind, action, message: compacted };
}
