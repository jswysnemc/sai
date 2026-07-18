import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { buildAgentChoices, resolveAgentChoice } from "./agent-options";
import { useI18n } from "../i18n/use-i18n";

const STORAGE_KEY = "sai.chat-agent";

/**
 * 管理主界面 Agent 列表、当前选择和本地偏好。
 *
 * @returns Agent 列表、当前选择、加载状态和更新方法
 */
export function useChatAgent() {
  const { locale } = useI18n();
  const response = useQuery({ queryKey: ["config"], queryFn: api.config.load });
  const [preferredId, setPreferredId] = useState(() => window.localStorage.getItem(STORAGE_KEY));
  const choices = response.data ? buildAgentChoices(response.data.config, locale) : [];
  // 本地未选过时，跟随配置里的 Web 默认 Agent（default_agent），而不是写死 general
  const fallbackId = response.data?.config.default_agent?.trim() || "general";
  const selection = resolveAgentChoice(choices, preferredId ?? fallbackId);

  useEffect(() => {
    if (selection) window.localStorage.setItem(STORAGE_KEY, selection.id);
  }, [selection?.id]);

  return {
    choices,
    selection,
    selectAgent: setPreferredId,
    isLoading: response.isLoading,
    error: response.error
  };
}
