import type { AppConfig } from "../../api/contracts";
import { Select } from "../../shared/ui/select/select";
import { buildChatModelChoices } from "../chat/chat-model-options";
import { useI18n } from "../i18n/use-i18n";

const INHERIT_VALUE = "";

type MemoryExtractionModelFieldProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染记忆提取模型选择器；空值表示跟随当前会话模型。
 *
 * @param props 应用配置和更新回调
 * @returns 记忆提取模型设置字段
 */
export function MemoryExtractionModelField({ config, onConfigChange }: MemoryExtractionModelFieldProps) {
  const { t } = useI18n();
  const memory = (config.memory as { extraction_provider_id?: string; extraction_model?: string } | undefined) ?? {};
  const current = memory.extraction_provider_id && memory.extraction_model
    ? encodeChoice(memory.extraction_provider_id, memory.extraction_model)
    : INHERIT_VALUE;
  const options = [
    {
      value: INHERIT_VALUE,
      label: t("Follow conversation model", "跟随会话模型"),
      description: t("Use the model selected by the current conversation for each memory extraction", "每次记忆提取使用当前会话实际选择的模型")
    },
    ...buildChatModelChoices(config).map((choice) => ({
      value: encodeChoice(choice.providerId, choice.model),
      label: `${choice.providerName} / ${choice.model}`,
      description: t("Always use this model to extract session memory points", "始终使用该模型提取会话记忆点")
    }))
  ];

  /** 更新记忆提取模型配置。 */
  const update = (value: string) => {
    const [providerId = "", model = ""] = value ? value.split("\u0000", 2) : [];
    onConfigChange({
      ...config,
      memory: {
        ...(config.memory as Record<string, unknown> | undefined),
        extraction_provider_id: providerId,
        extraction_model: model
      }
    });
  };

  return (
    <label className="settings-field">
      <span>{t("Session memory model", "记忆提取模型")}</span>
      <Select
        value={current}
        options={options}
        ariaLabel={t("Choose session memory extraction model", "选择会话记忆提取模型")}
        menuPreferredWidth={360}
        menuMinimumWidth={280}
        onChange={update}
      />
      <small>{t("An empty value follows the current conversation model", "留空时自动跟随当前会话模型")}</small>
    </label>
  );
}

/** 编码供应商与模型为选择器值。 */
function encodeChoice(providerId: string, model: string): string {
  return `${providerId}\u0000${model}`;
}
