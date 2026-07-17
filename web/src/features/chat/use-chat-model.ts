import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { RunModelSelection } from "../../api/contracts";
import { api } from "../../api/client";
import { buildChatModelChoices, resolveChatModelSelection } from "./chat-model-options";

const GLOBAL_KEY = "sai.chat-model";
const sessionKey = (sessionId?: string) => (sessionId ? `sai.chat-model.${sessionId}` : GLOBAL_KEY);

/**
 * 管理输入区模型列表、当前选择和按会话隔离的本地偏好。
 *
 * @param sessionId 当前会话 ID；不同会话互不影响
 * @returns 模型查询状态、选项和选择方法
 */
export function useChatModel(sessionId?: string) {
  const response = useQuery({ queryKey: ["config"], queryFn: api.config.load });
  const [preferred, setPreferred] = useState<RunModelSelection | null>(() => loadStoredSelection(sessionId));
  const choices = response.data ? buildChatModelChoices(response.data.config) : [];
  const selection = response.data ? resolveChatModelSelection(response.data.config, preferred) : null;

  // 切换会话时恢复该会话自己的模型偏好
  useEffect(() => {
    setPreferred(loadStoredSelection(sessionId));
  }, [sessionId]);

  useEffect(() => {
    if (!selection) return;
    window.localStorage.setItem(sessionKey(sessionId), JSON.stringify(selection));
  }, [selection?.providerId, selection?.model, sessionId]);

  /** 更新当前会话使用的供应商和模型。 */
  const selectModel = (next: RunModelSelection) => setPreferred(next);

  return { choices, selection, selectModel, isLoading: response.isLoading, error: response.error };
}

function loadStoredSelection(sessionId?: string): RunModelSelection | null {
  try {
    const raw =
      window.localStorage.getItem(sessionKey(sessionId)) ??
      (sessionId ? window.localStorage.getItem(GLOBAL_KEY) : null);
    const value = JSON.parse(raw ?? "null") as Partial<RunModelSelection> | null;
    if (value?.providerId && value.model) return { providerId: value.providerId, model: value.model };
  } catch {
    return null;
  }
  return null;
}
