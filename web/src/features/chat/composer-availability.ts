import type { LiveRunState } from "./run-event-reducer";

export type ComposerAvailabilityInput = {
  sessionAvailable: boolean;
  runActive: boolean;
  runStatus: LiveRunState["status"];
  hasDraft: boolean;
};

export type ComposerAvailability = {
  inputDisabled: boolean;
  sendDisabled: boolean;
  showStop: boolean;
};

/**
 * 计算聊天输入区在当前运行阶段的可用状态。
 *
 * @param input 会话、运行阶段和草稿状态
 * @returns 输入、发送和停止按钮的可用状态
 */
export function resolveComposerAvailability(input: ComposerAvailabilityInput): ComposerAvailability {
  return {
    inputDisabled: !input.sessionAvailable,
    sendDisabled: !input.sessionAvailable || !input.hasDraft,
    showStop: input.runActive
  };
}
