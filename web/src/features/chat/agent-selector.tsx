import { Bot, Settings2 } from "lucide-react";
import { useState } from "react";
import type { AgentChoice } from "../agents/agent-types";
import { SubagentModelDialog } from "../agents/subagent-model-config/subagent-model-dialog";
import { Button } from "../../shared/ui/button/button";
import { Select } from "../../shared/ui/select/select";
import "./agent-selector.css";
import { useI18n } from "../i18n/use-i18n";

type AgentSelectorProps = {
  choices: AgentChoice[];
  selection: AgentChoice | null;
  loading: boolean;
  disabled: boolean;
  onSelect: (id: string) => void;
};

/**
 * 渲染主对话 Agent 选择器，并提供子任务模型与思考设置入口。
 *
 * @param props Agent 选项、当前选择、加载状态和更新回调
 * @returns Agent 单选控件与配置按钮
 */
export function AgentSelector({ choices, selection, loading, disabled, onSelect }: AgentSelectorProps) {
  const { t } = useI18n();
  const [quickConfigOpen, setQuickConfigOpen] = useState(false);
  return (
    <div className="agent-selector">
      <Bot size={13} aria-hidden />
      <Select
        value={selection?.id ?? ""}
        options={choices.map((choice) => ({ value: choice.id, label: choice.name }))}
        disabled={disabled || loading || choices.length === 0}
        ariaLabel={t("Choose Agent", "选择 Agent")}
        menuPreferredWidth={220}
        menuMinimumWidth={180}
        menuAlign="right"
        menuClassName="agent-selector-menu"
        onChange={onSelect}
      />
      <Button
        className="agent-selector-config"
        onClick={() => setQuickConfigOpen(true)}
        title={t("Subagent models & thinking", "子任务模型与思考")}
        aria-label={t("Subagent models & thinking", "子任务模型与思考")}
      >
        <Settings2 size={13} />
      </Button>
      {quickConfigOpen && (
        <SubagentModelDialog open onClose={() => setQuickConfigOpen(false)} />
      )}
    </div>
  );
}
