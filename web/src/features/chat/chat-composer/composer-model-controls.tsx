import { Cpu } from "lucide-react";
import type { EngineStatusResponse } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { AgentSelector } from "../agent-selector";
import { ModelThinkingSelector } from "../model-thinking-selector";
import { AcpRuntimeControls } from "../acp-runtime/acp-runtime-controls";
import { EngineConnectionBadge } from "../engine-connection/engine-connection-badge";
import type { ChatComposerProps } from "./composer-types";

type ComposerModelControlsProps = { composer: ChatComposerProps; engine: EngineStatusResponse | null; loading: boolean };

/**
 * 组合智能体、模型和思考设置，兼容外部内核的连接状态。
 * @param props 会话配置、外部内核状态与读取标记
 * @returns 输入框内的模型控制组
 */
export function ComposerModelControls({ composer, engine, loading }: ComposerModelControlsProps) {
  const { t } = useI18n();
  return (
    <div className="composer-model-controls">
      <AgentSelector choices={composer.agentChoices} selection={composer.agentSelection} loading={composer.agentLoading} disabled={false} onSelect={composer.onAgentSelect} />
      <span className="composer-control-divider" aria-hidden />
      {engine && composer.choices.length === 0 ? <EngineConnectionBadge status={engine} running={composer.running} />
        : loading ? <span className="composer-engine-badge"><Cpu size={13} />{t("Loading engine", "读取内核")}</span>
          : <ModelThinkingSelector choices={composer.choices} selection={composer.selection} pendingSelection={composer.pendingSelection} thinkingLevel={composer.thinkingLevel} thinkingLevels={composer.thinkingLevels} loading={composer.modelLoading} disabled={false} onModelSelect={composer.onModelSelect} onThinkingLevelChange={composer.onThinkingLevelChange} />}
      {engine && composer.choices.length > 0 && <AcpRuntimeControls status={engine} running={composer.running} />}
    </div>
  );
}
