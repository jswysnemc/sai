import { AgentSelector } from "../agent-selector";
import { ModelThinkingSelector } from "../model-thinking-selector";
import type { ChatComposerProps } from "./composer-types";

/** 【会话】【模型控制】组合代理、模型和思考选择；参数为输入区配置，返回控制组 */
export function ComposerModelControls({ composer }: { composer: ChatComposerProps }) {
  return (
    <div className="composer-model-controls">
      <AgentSelector choices={composer.agentChoices} selection={composer.agentSelection} loading={composer.agentLoading} disabled={false} onSelect={composer.onAgentSelect} />
      <span className="composer-control-divider" aria-hidden />
      <ModelThinkingSelector choices={composer.choices} selection={composer.selection} pendingSelection={composer.pendingSelection} thinkingLevel={composer.thinkingLevel} thinkingLevels={composer.thinkingLevels} loading={composer.modelLoading} disabled={false} onModelSelect={composer.onModelSelect} onThinkingLevelChange={composer.onThinkingLevelChange} />
    </div>
  );
}
