import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../../api/client";
import type { ModelEndpointConfig } from "../../../api/contracts";
import {
  imageGenerationEndpoints,
  readStoredImageEndpoint,
  resolveImageEndpoint,
  writeStoredImageEndpoint
} from "./image-generation-options";

/** 管理聊天页生图端点列表和按会话隔离的选中项。 */
export function useImageGenerationModel(sessionId?: string) {
  const config = useQuery({ queryKey: ["config"], queryFn: api.config.load });
  const [preference, setPreference] = useState<{ sessionId?: string; endpointId: string | null }>(() => ({
    sessionId,
    endpointId: readStoredImageEndpoint(sessionId)
  }));
  const endpoints = imageGenerationEndpoints(config.data?.config);
  const preferredId = preference.sessionId === sessionId
    ? preference.endpointId
    : readStoredImageEndpoint(sessionId);
  const selection = resolveImageEndpoint(endpoints, preferredId);

  useEffect(() => {
    setPreference({ sessionId, endpointId: readStoredImageEndpoint(sessionId) });
  }, [sessionId]);

  useEffect(() => {
    if (preference.sessionId !== sessionId) return;
    if (selection) writeStoredImageEndpoint(sessionId, selection.id);
  }, [preference.sessionId, selection?.id, sessionId]);

  /** 选择下一次图片请求使用的端点。 */
  const selectEndpoint = (endpointId: string) => {
    setPreference({ sessionId, endpointId });
    writeStoredImageEndpoint(sessionId, endpointId);
  };

  return {
    endpoints,
    selection,
    selectEndpoint,
    isLoading: config.isLoading,
    error: config.error
  };
}

/** 返回端点的显示模型名称，未填写时使用待配置提示。 */
export function imageEndpointModelLabel(endpoint: ModelEndpointConfig | null): string {
  return endpoint?.model?.trim() || "Model not set";
}
