import { Bot, Settings2 } from "lucide-react";
import { useState } from "react";
import type { AgentChoice } from "../agents/agent-types";
import { AgentConfigDialog } from "../agents/agent-config-dialog/agent-config-dialog";
import { Select } from "../../shared/ui/select/select";
import "./agent-selector.css";

type AgentSelectorProps = {
  choices: AgentChoice[];
  selection: AgentChoice | null;
  loading: boolean;
  disabled: boolean;
  onSelect: (id: string) => void;
};

/**
 * 渲染主界面 Agent 选择器,并提供打开全局 Agent 配置的入口。
 *
 * @param props Agent 选项、当前选择、加载状态和更新回调
 * @returns Agent 单选控件与配置按钮
 */
export function AgentSelector({ choices, selection, loading, disabled, onSelect }: AgentSelectorProps) {
  const [configOpen, setConfigOpen] = useState(false);
  return (
    <div className="agent-selector">
      <Bot size={13} aria-hidden />
      <Select
        value={selection?.id ?? ""}
        options={choices.map((choice) => ({ value: choice.id, label: choice.name }))}
        disabled={disabled || loading || choices.length === 0}
        ariaLabel="选择 Agent"
        menuPreferredWidth={220}
        menuMinimumWidth={180}
        menuAlign="right"
        menuClassName="agent-selector-menu"
        onChange={onSelect}
      />
      <button
        type="button"
        className="agent-selector-config"
        onClick={() => setConfigOpen(true)}
        title="Agent 配置"
        aria-label="打开 Agent 配置"
      >
        <Settings2 size={13} />
      </button>
      <AgentConfigDialog open={configOpen} onClose={() => setConfigOpen(false)} />
    </div>
  );
}
