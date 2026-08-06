import type { ChatModelChoice } from "../chat/chat-model-options";
import { ModelIcon } from "../../shared/ui/model-icon";
import type { SelectOption } from "../../shared/ui/select/select";

/**
 * 把会话模型条目转换为设置页统一下拉选项。
 *
 * 参数:
 * - `choice`: 供应商与模型组合
 * - `value`: 当前选择器使用的编码值
 * - `description`: 该设置项使用模型的说明
 * - `label`: 可选展示名称，默认显示供应商与模型
 *
 * 返回:
 * - 带模型图标的下拉选项
 */
export function modelSelectOption(
  choice: ChatModelChoice,
  value: string,
  description?: string,
  label?: string
): SelectOption<string> {
  return {
    value,
    label: label ?? `${choice.providerName} / ${choice.model}`,
    description,
    icon: <ModelIcon model={choice.model} provider={choice.providerId} size={14} />
  };
}
