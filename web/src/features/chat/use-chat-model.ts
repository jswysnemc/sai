import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { RunModelSelection } from "../../api/contracts";
import { api } from "../../api/client";
import { buildChatModelChoices, resolveChatModelSelection } from "./chat-model-options";
import { isSameModelSelection, resolveModelSelect } from "./pending-model-selection";
import { modelThinkingLevels } from "./model-thinking-availability";
import {
  readStoredChatModelSelection,
  writeStoredChatModelSelection
} from "./session-preference-storage";

type StoredModelPreference = {
  sessionId?: string;
  selection: RunModelSelection | null;
};

/**
 * 【会话】【模型偏好】管理输入区模型列表、当前选择和按会话隔离的本地偏好。
 *
 * 运行中点选的新模型不打断当前 turn：先暂存为待生效选择，
 * 待本会话全部运行结束后自动应用为当前模型。
 *
 * @param sessionId 当前会话 ID；不同会话互不影响
 * @param running 当前会话是否有运行中的 turn；缺省视为空闲
 * @returns 模型查询状态、选项、待生效选择和选择方法
 */
export function useChatModel(sessionId?: string, running = false) {
  const response = useQuery({ queryKey: ["config"], queryFn: api.config.load });
  const [storedPreference, setStoredPreference] = useState<StoredModelPreference>(() => ({
    sessionId,
    selection: readStoredChatModelSelection(sessionId)
  }));
  // 运行中点选的待生效模型；带会话标识，切换会话后不误应用
  const [pendingPreference, setPendingPreference] = useState<StoredModelPreference | null>(null);
  const preferred = storedPreference.sessionId === sessionId
    ? storedPreference.selection
    : readStoredChatModelSelection(sessionId);
  const choices = response.data ? buildChatModelChoices(response.data.config) : [];
  const selection = response.data ? resolveChatModelSelection(response.data.config, preferred) : null;

  // 1. 【会话】【模型偏好】切换会话时恢复该会话自己的模型偏好
  useEffect(() => {
    setStoredPreference((current) => current.sessionId === sessionId
      ? current
      : { sessionId, selection: readStoredChatModelSelection(sessionId) });
  }, [sessionId]);

  useEffect(() => {
    if (!selection) return;
    writeStoredChatModelSelection(sessionId, selection);
  }, [selection?.providerId, selection?.model, sessionId]);

  // 2. 【会话】【待生效模型】本会话全部运行结束后自动应用待生效选择
  useEffect(() => {
    if (running) return;
    if (!pendingPreference || pendingPreference.sessionId !== sessionId) return;
    setStoredPreference(pendingPreference);
    setPendingPreference(null);
  }, [pendingPreference, running, sessionId]);

  /**
   * 【会话】【模型偏好】更新当前会话使用的供应商和模型。
   *
   * 运行中不打断当前 turn：新选择暂存为待生效，本轮结束后自动应用；
   * 运行中点回当前生效模型则撤销暂存。
   *
   * @param next 新模型选择
   * @returns 无返回值
   */
  const selectModel = (next: RunModelSelection) => {
    const action = resolveModelSelect(running, selection, next);
    if (action.kind === "apply") {
      setPendingPreference(null);
      setStoredPreference({ sessionId, selection: action.selection });
      return;
    }
    setPendingPreference(action.kind === "stage" ? { sessionId, selection: action.selection } : null);
  };

  // 待生效选择解析为完整选项；与当前生效模型一致时不再标注
  const pendingCandidate = pendingPreference && pendingPreference.sessionId === sessionId
    ? pendingPreference.selection
    : null;
  const pendingSelection = pendingCandidate && !isSameModelSelection(pendingCandidate, selection)
    ? choices.find((choice) => isSameModelSelection(choice, pendingCandidate)) ?? null
    : null;

  return {
    choices,
    selection,
    pendingSelection,
    thinkingLevels: modelThinkingLevels(response.data?.config, selection),
    selectModel,
    isLoading: response.isLoading,
    error: response.error
  };
}
