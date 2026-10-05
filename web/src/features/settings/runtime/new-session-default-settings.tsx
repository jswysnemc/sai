import type {
  AppConfig,
  EngineStatusResponse,
  ThinkingLevel
} from "../../../api/contracts";
import { FieldGrid, SettingsField, SkSelect } from "../kit";
import { THINKING_OPTIONS } from "../../chat/model-thinking-options";
import {
  buildNewSessionModelChoices,
  buildNewSessionThinkingLevels,
  disabledNewSessionProviderName,
  resolveConfiguredNewSessionPreferences
} from "../../sessions/new-session-preferences";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { modelSelectOption } from "../model-select-option";

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
  const disabledProviderName = disabledNewSessionProviderName(config);
  const configuredModelValue = preferences.model
    ? encodeModelChoice(preferences.model.providerId, preferences.model.model)
    : DEFAULT_MODEL_VALUE;
  // 1. 停用供应商后仍保留当前配置项，避免界面静默回落到「跟随内核默认」而草稿仍是原值
  const staleChoice = disabledProviderName && preferences.model
    ? {
      providerId: preferences.model.providerId,
      providerName: disabledProviderName,
      model: preferences.model.model
    }
    : null;
  const visibleChoices = staleChoice
    && !modelChoices.some((choice) => choice.providerId === staleChoice.providerId && choice.model === staleChoice.model)
    ? [staleChoice, ...modelChoices]
    : modelChoices;
  const modelOptions = [
    {
      value: DEFAULT_MODEL_VALUE,
      label: t("Follow engine default", "跟随内核默认模型"),
      description: t(
        "Use the model currently configured for the selected conversation engine.",
        "使用当前对话内核配置的默认模型。"
      )
    },
    ...visibleChoices.map((choice) => modelSelectOption(
      choice,
      encodeModelChoice(choice.providerId, choice.model),
      staleChoice && choice.providerId === staleChoice.providerId && choice.model === staleChoice.model
        ? t("This provider is disabled; pick another model or follow the engine default.", "该供应商已停用；请另选模型或改回跟随内核默认。")
        : t(
          "Start each new session with this model.",
          "每个新会话初始使用此模型。"
        ),
      external ? choice.model : undefined
    ))
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
    <FieldGrid>
      <SettingsField
        label={t("New session model", "新会话模型")}
        configKey="session.new_session_model"
        anchor="runtime.session.new_session_model"
        hint={t("Applied only when creating a session.", "仅在创建新会话时应用。")}
        error={disabledProviderName
          ? t(
            `The selected provider (${disabledProviderName}) is disabled, so this default cannot be saved. Choose another model or follow the engine default.`,
            `所选供应商（${disabledProviderName}）已停用，无法保存该默认模型。请另选模型，或改回跟随内核默认。`
          )
          : undefined}
        aside={disabledProviderName
          ? <Button size="small" variant="ghost" onClick={() => updateModel(DEFAULT_MODEL_VALUE)}>{t("Follow engine default", "跟随内核默认")}</Button>
          : undefined}
      >

        <SkSelect
          value={modelValue}
          options={modelOptions}
          onChange={updateModel}
          ariaLabel={t("New session model", "新会话模型")}
          menuPreferredWidth={380}
          menuMinimumWidth={280}
        />
      </SettingsField>
      <SettingsField label={t("New session reasoning effort", "新会话思考等级")} configKey="session.new_session_thinking_level" anchor="runtime.session.new_session_thinking_level" hint={t("Auto uses the provider or ACP engine default.", "自动模式使用供应商或 ACP 内核的默认思考行为。")}>

        <SkSelect
          value={thinkingValue}
          options={thinkingOptions}
          onChange={updateThinkingLevel}
          ariaLabel={t("New session reasoning effort", "新会话思考等级")}
          menuPreferredWidth={340}
          menuMinimumWidth={260}
        />
      </SettingsField>
    </FieldGrid>
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
