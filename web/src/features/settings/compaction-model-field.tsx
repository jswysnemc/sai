import type { AppConfig } from "../../api/contracts";
import { Select } from "../../shared/ui/select/select";
import { buildChatModelChoices } from "../chat/chat-model-options";

const INHERIT_VALUE = "";

type CompactionModelFieldProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染压缩模型选择器；空值表示跟随当前会话模型。
 *
 * @param props 应用配置和更新回调
 * @returns 压缩模型设置字段
 */
export function CompactionModelField({ config, onConfigChange }: CompactionModelFieldProps) {
  const context = config.context ?? { default_max_chars: 120_000 };
  const current = context.compaction_provider_id && context.compaction_model
    ? encodeChoice(context.compaction_provider_id, context.compaction_model)
    : INHERIT_VALUE;
  const options = [
    {
      value: INHERIT_VALUE,
      label: "跟随会话模型",
      description: "每次压缩使用当前会话实际选择的模型"
    },
    ...buildChatModelChoices(config).map((choice) => ({
      value: encodeChoice(choice.providerId, choice.model),
      label: `${choice.providerName} / ${choice.model}`,
      description: "始终使用该模型生成压缩摘要"
    }))
  ];

  /** 更新压缩模型配置。 */
  const update = (value: string) => {
    const [providerId = "", model = ""] = value ? value.split("\u0000", 2) : [];
    onConfigChange({
      ...config,
      context: {
        ...context,
        compaction_provider_id: providerId,
        compaction_model: model
      }
    });
  };

  return (
    <label className="settings-field">
      <span>压缩模型</span>
      <Select
        value={current}
        options={options}
        ariaLabel="选择上下文压缩模型"
        menuPreferredWidth={360}
        menuMinimumWidth={280}
        onChange={update}
      />
      <small>留空时自动跟随当前会话模型</small>
    </label>
  );
}

/** 编码供应商与模型为选择器值。 */
function encodeChoice(providerId: string, model: string): string {
  return `${providerId}\u0000${model}`;
}
