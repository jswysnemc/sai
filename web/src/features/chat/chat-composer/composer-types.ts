import type { RunMode, RunModelSelection, ThinkingLevel } from "../../../api/contracts";
import type { ChatModelChoice } from "../chat-model-options";
import type { ComposerAttachment } from "../composer/use-composer-attachments";
import type { LiveRunState } from "../run-event-reducer";
import type { AgentChoice } from "../../agents/agent-types";

export type ChatComposerProps = {
  value: string;
  mode: RunMode;
  attachments: ComposerAttachment[];
  historyEntries: string[];
  thinkingLevel: ThinkingLevel;
  thinkingLevels?: ThinkingLevel[];
  choices: ChatModelChoice[];
  selection: ChatModelChoice | null;
  /** 运行中点选的待生效模型；本轮结束后自动应用 */
  pendingSelection: ChatModelChoice | null;
  modelLoading: boolean;
  running: boolean;
  /** 分支切换等短暂过渡期仅禁止发送，草稿仍可编辑。 */
  submitBlocked?: boolean;
  runStatus: LiveRunState["status"];
  sessionAvailable: boolean;
  undoAvailable: boolean;
  agentChoices: AgentChoice[];
  agentSelection: AgentChoice | null;
  agentLoading: boolean;
  sessionId?: string;
  submitting?: boolean;
  onChange: (value: string) => void;
  onModeChange: (mode: RunMode) => void;
  onThinkingLevelChange: (level: ThinkingLevel) => void;
  onAddImages: (files: File[], selectionStart: number, selectionEnd: number) => Promise<number | undefined>;
  onRemoveAttachment: (id: number) => void;
  onModelSelect: (selection: RunModelSelection) => void;
  onSubmit: () => void;
  onStop: () => void;
  onUndo: () => void;
  onAgentSelect: (id: string) => void;
  onCompact: () => Promise<void>;
  onContinueGoal: () => Promise<void>;
};
