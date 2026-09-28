import { useMemo } from "react";
import type { AppConfig } from "../../../api/contracts";
import { buildChatModelChoices } from "../../chat/chat-model-options";
import { SkSelect, type SelectOption } from "../kit";
import { modelSelectOption } from "../model-select-option";
import { decodeModelChoice, encodeModelChoice, INHERIT_MODEL_VALUE } from "./model-choice";

type ModelChoiceSelectProps = {
  config: AppConfig;
  providerId?: string | null;
  model?: string | null;
  disabled?: boolean;
  /** 「跟随当前模型」选项的名称 */
  inheritLabel: string;
  /** 「跟随当前模型」选项的说明 */
  inheritDescription?: string;
  /** 具体模型选项的说明 */
  optionDescription?: string;
  onChange: (providerId: string, model: string) => void;
};

/**
 * 渲染模型选择：首项为跟随当前模型，其后为全部已启用供应商的模型。
 *
 * 提交说明、上下文压缩、记忆提取等辅助任务共用此选择器。
 *
 * @param props 应用配置、当前选择、选项文案与更新回调
 * @returns 模型下拉选择
 */
export function ModelChoiceSelect({
  config,
  providerId,
  model,
  disabled,
  inheritLabel,
  inheritDescription,
  optionDescription,
  onChange
}: ModelChoiceSelectProps) {
  const options = useMemo<SelectOption<string>[]>(() => [
    { value: INHERIT_MODEL_VALUE, label: inheritLabel, description: inheritDescription },
    ...buildChatModelChoices(config).map((choice) => modelSelectOption(
      choice,
      encodeModelChoice(choice.providerId, choice.model),
      optionDescription
    ))
  ], [config, inheritDescription, inheritLabel, optionDescription]);
  return (
    <SkSelect
      disabled={disabled}
      value={encodeModelChoice(providerId, model)}
      options={options}
      menuPreferredWidth={360}
      menuMinimumWidth={280}
      onChange={(value) => {
        const next = decodeModelChoice(value);
        onChange(next.providerId, next.model);
      }}
    />
  );
}
