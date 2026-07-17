import { Select } from "../../../shared/ui/select/select";
import type { AgentChoice } from "../agent-types";

type DefaultAgentPickerProps = {
  choices: AgentChoice[];
  value: string;
  onChange: (id: string) => void;
};

/**
 * 渲染全局默认 Agent 单选列表。
 *
 * @param props Agent 选项、当前值与变化回调
 * @returns 默认 Agent 选择区
 */
export function DefaultAgentPicker({ choices, value, onChange }: DefaultAgentPickerProps) {
  return (
    <Select
      value={value}
      options={choices.map((choice) => ({ value: choice.id, label: choice.name }))}
      onChange={onChange}
      ariaLabel="选择默认 Agent"
      menuPreferredWidth={280}
      menuMinimumWidth={220}
    />
  );
}
