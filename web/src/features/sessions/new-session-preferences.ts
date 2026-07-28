import type {
  AppConfig,
  EngineStatusResponse,
  RunModelSelection,
  ThinkingLevel
} from "../../api/contracts";
import {
  ACP_PROVIDER_ID,
  acpThinkingLevels,
  advertisedAcpModelChoices,
  buildAcpModelChoices
} from "../chat/acp-engine-options";
import { buildChatModelChoices } from "../chat/chat-model-options";
import type { ChatModelChoice } from "../chat/chat-model-options";
import {
  normalizeThinkingLevel,
  writeStoredChatModelSelection,
  writeStoredThinkingLevel
} from "../chat/session-preference-storage";
import { THINKING_OPTIONS } from "../chat/model-thinking-options";

export type NewSessionPreferences = {
  model: RunModelSelection | null;
  thinkingLevel: ThinkingLevel;
};

/**
 * 【会话】【新会话默认值】解析配置中显式指定的新会话偏好。
 *
 * @param config 应用配置
 * @returns 模型选择和思考等级；模型留空时返回 null
 */
export function resolveConfiguredNewSessionPreferences(config: AppConfig): NewSessionPreferences {
  const providerId = config.session?.new_session_provider_id?.trim() ?? "";
  const model = config.session?.new_session_model?.trim() ?? "";
  return {
    model: providerId && model ? { providerId, model } : null,
    thinkingLevel: normalizeThinkingLevel(config.session?.new_session_thinking_level)
  };
}

/**
 * 【会话】【新会话默认值】返回当前内核可用于新会话默认值的模型。
 *
 * @param config 应用配置草稿
 * @param status 当前内核运行状态
 * @returns 普通供应商或 ACP 模型列表
 */
export function buildNewSessionModelChoices(
  config: AppConfig,
  status?: EngineStatusResponse
): ChatModelChoice[] {
  const engine = config.agent?.engine ?? "native";
  if (engine === "native") {
    return Array.isArray(config.providers) ? buildChatModelChoices(config) : [];
  }

  const matchingStatus = status?.engine === engine ? status : undefined;
  if (matchingStatus?.acp_runtime) {
    return advertisedAcpModelChoices(matchingStatus);
  }
  const choices = buildAcpModelChoices(matchingStatus, config);
  const configured = resolveConfiguredNewSessionPreferences(config).model;
  if (
    configured?.providerId === ACP_PROVIDER_ID
    && !choices.some((choice) => choice.model === configured.model)
  ) {
    return [
      ...choices,
      {
        providerId: ACP_PROVIDER_ID,
        providerName: matchingStatus?.label ?? "ACP",
        model: configured.model
      }
    ];
  }
  return choices;
}

/**
 * 【会话】【新会话默认值】返回当前内核可实际选择的思考等级。
 *
 * @param config 应用配置草稿
 * @param status 当前内核运行状态
 * @returns 内置内核的完整等级，或 ACP 明确支持的等级与 auto
 */
export function buildNewSessionThinkingLevels(
  config: AppConfig,
  status?: EngineStatusResponse
): ThinkingLevel[] {
  const engine = config.agent?.engine ?? "native";
  if (engine === "native") return THINKING_OPTIONS.map((option) => option.value);
  const matchingStatus = status?.engine === engine ? status : undefined;
  if (!matchingStatus?.acp_runtime) return THINKING_OPTIONS.map((option) => option.value);
  return Array.from(new Set<ThinkingLevel>(["auto", ...acpThinkingLevels(matchingStatus)]));
}

/**
 * 【会话】【新会话默认值】解析当前内核能够实际应用的新会话偏好。
 *
 * ACP 已公布能力快照时只接受其中明确支持的模型与思考等级；首次握手前保留
 * 用户配置，已有能力快照中的失效配置则回退内核默认值。
 *
 * @param config 应用配置
 * @param status 当前内核运行状态
 * @returns 当前内核可以应用的模型与思考偏好
 */
export function resolveEffectiveNewSessionPreferences(
  config: AppConfig,
  status?: EngineStatusResponse
): NewSessionPreferences {
  const configured = resolveConfiguredNewSessionPreferences(config);
  const engine = config.agent?.engine ?? "native";
  if (engine === "native") return configured;

  const matchingStatus = status?.engine === engine ? status : undefined;
  if (!matchingStatus?.acp_runtime) return configured;

  const advertisedModels = advertisedAcpModelChoices(matchingStatus);
  const model = configured.model?.providerId === ACP_PROVIDER_ID
    && advertisedModels.some((choice) => choice.model === configured.model?.model)
    ? configured.model
    : null;
  const advertisedThinkingLevels = acpThinkingLevels(matchingStatus);
  const thinkingLevel = configured.thinkingLevel === "auto"
    || advertisedThinkingLevels.includes(configured.thinkingLevel)
    ? configured.thinkingLevel
    : "auto";
  return { model, thinkingLevel };
}

/**
 * 【会话】【新会话默认值】为刚创建的会话建立专属模型与思考偏好。
 *
 * 模型为空时写入显式 null，避免新会话继承旧的全局模型选择；聊天 Hook
 * 随后会按当前内核默认值解析并保存实际模型。
 *
 * @param sessionId 新会话 ID
 * @param config 创建时生效的应用配置
 * @param status 创建时的内核运行状态
 * @returns 已写入的偏好值
 */
export function initializeNewSessionPreferences(
  sessionId: string,
  config: AppConfig,
  status?: EngineStatusResponse
): NewSessionPreferences {
  const preferences = resolveEffectiveNewSessionPreferences(config, status);
  writeStoredChatModelSelection(sessionId, preferences.model);
  writeStoredThinkingLevel(sessionId, preferences.thinkingLevel);
  return preferences;
}

/**
 * 【会话】【新会话默认值】清除配置中的新会话模型引用。
 *
 * @param session 当前会话配置
 * @returns 已清除供应商与模型字段的会话配置
 */
export function clearNewSessionModelReference(
  session: AppConfig["session"]
): NonNullable<AppConfig["session"]> {
  return {
    ...session,
    new_session_provider_id: undefined,
    new_session_model: undefined
  };
}

/**
 * 【会话】【新会话默认值】切换内核时重置与内核能力相关的默认值。
 *
 * @param session 当前会话配置
 * @returns 模型已清除且思考等级恢复 auto 的会话配置
 */
export function resetNewSessionEnginePreferences(
  session: AppConfig["session"]
): NonNullable<AppConfig["session"]> {
  return {
    ...clearNewSessionModelReference(session),
    new_session_thinking_level: "auto"
  };
}

/**
 * 【会话】【新会话默认值】供应商标识变化时同步更新新会话模型引用。
 *
 * @param session 当前会话配置
 * @param previousId 修改前的供应商标识
 * @param nextId 修改后的供应商标识
 * @returns 已同步供应商标识的会话配置
 */
export function renameNewSessionProviderReference(
  session: AppConfig["session"],
  previousId: string | undefined,
  nextId: string
): AppConfig["session"] {
  if (!session || !previousId || session.new_session_provider_id !== previousId) return session;
  return { ...session, new_session_provider_id: nextId };
}
