import type {
  AppConfig,
  EngineStatusResponse,
  ThinkingLevel
} from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
import { THINKING_OPTIONS } from "../../chat/model-thinking-options";
import {
  buildNewSessionModelChoices,
  buildNewSessionThinkingLevels,
  resolveConfiguredNewSessionPreferences
} from "../../sessions/new-session-preferences";
import { useI18n } from "../../i18n/use-i18n";

const DEFAULT_MODEL_VALUE = "";

type NewSessionDefaultSettingsProps = {
  config: AppConfig;
  status?: EngineStatusResponse;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 【设置】【新会话默认值】渲染仅对后续新会话生效的模型与思考等级。
 *
 * @param props 应用配置、当前内核状态和更新回调
 * @returns 新会话默认值表单
 */
export function NewSessionDefaultSettings({
  config,
  status,
  onConfigChange
}: NewSessionDefaultSettingsProps) {
  const { t } = useI18n();
  const engine = config.agent?.engine ?? "native";
  const external = engine !== "native";
  const matchingStatus = status?.engine === engine ? status : undefined;
  const preferences = resolveConfiguredNewSessionPreferences(config);
  const modelChoices = buildNewSessionModelChoices(config, matchingStatus);
  const configuredModelValue = preferences.model
    ? encodeModelChoice(preferences.model.providerId, preferences.model.model)
    : DEFAULT_MODEL_VALUE;
  const modelOptions = [
    {
      value: DEFAULT_MODEL_VALUE,
      label: t("Follow engine default", "跟随内核默认模型"),
      description: t(
        "Use the model currently configured for the selected conversation engine.",
        "使用当前对话内核配置的默认模型。"
      )
    },
    ...modelChoices.map((choice) => ({
      value: encodeModelChoice(choice.providerId, choice.model),
      label: external ? choice.model : `${choice.providerName} / ${choice.model}`,
      description: t(
        "Start each new session with this model.",
        "每个新会话初始使用此模型。"
      )
    }))
  ];
  const modelValue = modelOptions.some((option) => option.value === configuredModelValue)
    ? configuredModelValue
    : DEFAULT_MODEL_VALUE;
  const selectableThinkingLevels = new Set(
    buildNewSessionThinkingLevels(config, matchingStatus)
  );
  const thinkingOptions = THINKING_OPTIONS
    .filter((option) => selectableThinkingLevels.has(option.value))
    .map((option) => ({
      value: option.value,
      label: option.label,
      description: t(option.descriptionEn, option.descriptionZh)
    }));
  const thinkingValue = thinkingOptions.some((option) => option.value === preferences.thinkingLevel)
    ? preferences.thinkingLevel
    : "auto";

  /**
   * 【设置】【新会话默认值】合并新会话配置补丁并保留自动标题字段。
   *
   * @param patch 新会话配置局部更新
   * @returns 无返回值
   */
  const patchSession = (patch: Partial<NonNullable<AppConfig["session"]>>) => {
    onConfigChange({
      ...config,
      session: {
        ...config.session,
        ...patch
      }
    });
  };

  /**
   * 【设置】【新会话默认值】更新新会话模型；空值表示跟随当前内核默认模型。
   *
   * @param value 编码后的供应商与模型
   * @returns 无返回值
   */
  const updateModel = (value: string) => {
    const [providerId = "", model = ""] = value ? value.split("\u0000", 2) : [];
    patchSession({
      new_session_provider_id: providerId || undefined,
      new_session_model: model || undefined
    });
  };

  /**
   * 【设置】【新会话默认值】更新新会话思考等级。
   *
   * @param value 当前内核支持的思考等级
   * @returns 无返回值
   */
  const updateThinkingLevel = (value: ThinkingLevel) => {
    patchSession({ new_session_thinking_level: value });
  };

  return (
    <div className="settings-form-grid">
      <div className="settings-field">
        <span>{t("New session model", "新会话模型")}</span>
        <Select
          value={modelValue}
          options={modelOptions}
          onChange={updateModel}
          ariaLabel={t("New session model", "新会话模型")}
          menuPreferredWidth={380}
          menuMinimumWidth={280}
        />
        <small>{t(
          "Applied only when a session is created; existing session choices stay unchanged.",
          "仅在创建会话时应用，现有会话的选择保持不变。"
        )}</small>
      </div>
      <div className="settings-field">
        <span>{t("New session reasoning effort", "新会话思考等级")}</span>
        <Select
          value={thinkingValue}
          options={thinkingOptions}
          onChange={updateThinkingLevel}
          ariaLabel={t("New session reasoning effort", "新会话思考等级")}
          menuPreferredWidth={340}
          menuMinimumWidth={260}
        />
        <small>{t(
          "Auto lets the provider or ACP agent choose its default reasoning behavior.",
          "auto 表示由供应商或 ACP 内核采用默认思考行为。"
        )}</small>
      </div>
    </div>
  );
}

/**
 * 【设置】【新会话默认值】编码供应商与模型为下拉框内部值。
 *
 * @param providerId 供应商标识
 * @param model 模型标识
 * @returns 可逆的下拉框值
 */
function encodeModelChoice(providerId: string, model: string): string {
  return `${providerId}\u0000${model}`;
}
