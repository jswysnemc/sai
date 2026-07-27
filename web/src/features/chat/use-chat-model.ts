import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { RunModelSelection } from "../../api/contracts";
import { api } from "../../api/client";
import { acpThinkingLevels, buildAcpModelChoices, currentAcpModel } from "./acp-engine-options";
import { buildChatModelChoices, resolveChatModelSelection } from "./chat-model-options";
import type { ChatModelChoice } from "./chat-model-options";

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
  const engineStatus = useQuery({
    queryKey: ["engine-status"],
    queryFn: api.config.engineStatus,
    refetchInterval: (query) => query.state.data?.external && !query.state.data.acp_runtime ? 1_000 : false
  });
  const [preferred, setPreferred] = useState<RunModelSelection | null>(() => loadStoredSelection(sessionId));
  const external = engineStatus.data?.external === true;
  const choices = response.data
    ? external
      ? buildAcpModelChoices(engineStatus.data, response.data.config)
      : buildChatModelChoices(response.data.config)
    : [];
  const selection = response.data
    ? external
      ? resolveExternalSelection(choices, preferred, currentAcpModel(engineStatus.data))
      : resolveChatModelSelection(response.data.config, preferred)
    : null;

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

  return {
    choices,
    selection,
    thinkingLevels: external ? acpThinkingLevels(engineStatus.data) : undefined,
    isExternal: external,
    selectModel,
    isLoading: response.isLoading || engineStatus.isLoading,
    error: response.error ?? engineStatus.error
  };
}

/**
 * 解析外部内核当前模型，优先使用本会话选择。
 *
 * @param choices agent 公布的模型
 * @param preferred 浏览器保存的会话偏好
 * @param current agent 当前模型
 * @returns 可用的 ACP 模型选择
 */
function resolveExternalSelection(
  choices: ChatModelChoice[],
  preferred: RunModelSelection | null,
  current: string | undefined
): ChatModelChoice | null {
  return choices.find((choice) => choice.providerId === preferred?.providerId && choice.model === preferred?.model)
    ?? choices.find((choice) => choice.model === current)
    ?? choices[0]
    ?? null;
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
