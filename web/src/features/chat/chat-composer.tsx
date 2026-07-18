import { Activity, ArrowRight, Bot, GitBranch, Paperclip, Square, Undo2 } from "lucide-react";
import { useRef } from "react";
import type { ChangeEvent, FormEvent } from "react";
import type { RunMode, RunModelSelection, ThinkingLevel } from "../../api/contracts";
import type { ChatModelChoice } from "./chat-model-options";
import { AttachmentStrip } from "./composer/attachment-strip";
import { ComposerTextarea } from "./composer/composer-textarea";
import type { ComposerAttachment } from "./composer/use-composer-attachments";
import { resolveComposerAvailability } from "./composer-availability";
import { ModelThinkingSelector } from "./model-thinking-selector";
import type { LiveRunState } from "./run-event-reducer";
import type { AgentChoice } from "../agents/agent-types";
import { AgentSelector } from "./agent-selector";
import { WorkspaceSwitcher } from "../workspaces/workspace-switcher";
import { SystemUsage } from "../usage/system-usage";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import { TodoMarkdownView } from "../todo/todo-markdown-view";
import { useRuntimeActivity } from "../runtime-activity/use-runtime-activity";
import { PermissionAuditDialog } from "../permission/permission-audit-dialog";
import { Button } from "../../shared/ui/button/button";
import { Select } from "../../shared/ui/select/select";
import { useI18n } from "../i18n/use-i18n";
import { GoalControl } from "../goals/goal-control";
import "./chat-composer.css";

type ChatComposerProps = {
  value: string;
  mode: RunMode;
  attachments: ComposerAttachment[];
  historyEntries: string[];
  thinkingLevel: ThinkingLevel;
  choices: ChatModelChoice[];
  selection: ChatModelChoice | null;
  modelLoading: boolean;
  running: boolean;
  runStatus: LiveRunState["status"];
  sessionAvailable: boolean;
  undoAvailable: boolean;
  agentChoices: AgentChoice[];
  agentSelection: AgentChoice | null;
  agentLoading: boolean;
  sessionId?: string;
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

/**
 * 渲染 sai-chat 风格的底部输入区。
 *
 * @param props 输入状态、模型状态、附件状态和操作回调
 * @returns 聊天输入区
 */
export function ChatComposer(props: ChatComposerProps) {
  const { t } = useI18n();
  const git = useQuery({ queryKey:["git-status"], queryFn:api.workspace.gitStatus, staleTime:20_000 });
  const runtimeActivity = useRuntimeActivity();
  const fileInputRef = useRef<HTMLInputElement>(null);

  /**
   * 提交当前输入内容。
   *
   * @param event 表单提交事件
   */
  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (availability.sendDisabled) return;
    props.onSubmit();
  };

  /**
   * 读取文件选择器中的全部图片。
   *
   * @param event 文件输入变更事件
   */
  const handleFileChange = (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length === 0) return;
    void props.onAddImages(files, props.value.length, props.value.length);
  };

  const availability = resolveComposerAvailability({
    sessionAvailable: props.sessionAvailable,
    runActive: props.running,
    runStatus: props.runStatus,
    hasDraft: Boolean(props.value.trim()) || props.attachments.length > 0
  });
  const runModeOptions = [
    { value: "yolo", label: t("Work", "工作"), description: t("Execute directly without per-tool permission prompts", "直接执行，不逐次询问工具权限") },
    { value: "audited", label: t("Audited", "审核"), description: t("Ask before write tools and restrict them to the workspace sandbox", "写入工具逐次询问，限制在工作区沙盒") },
    { value: "plan", label: t("Plan", "规划"), description: t("Use read-only tools and prohibit modifications", "仅只读工具，禁止修改与写操作") }
  ] satisfies Array<{ value: RunMode; label: string; description: string }>;

  return (
    <div className="composer-shell">
      <div className="composer-context-strip">
        <WorkspaceSwitcher />
        {git.data?.status === "ready" && git.data.head && <span className="composer-context-chip" title={git.data.upstream || git.data.head}><GitBranch size={13}/><span>{git.data.head}</span></span>}
        <SystemUsage selection={props.selection} onCompact={props.onCompact} compactDisabled={props.running} />
        <AgentSelector choices={props.agentChoices} selection={props.agentSelection} loading={props.agentLoading} disabled={props.running} onSelect={props.onAgentSelect} />
        <PermissionAuditDialog sessionId={props.sessionId} />
        <GoalControl sessionId={props.sessionId} running={props.running} onContinue={props.onContinueGoal} />
        <Button className="composer-rail-button" onClick={props.onUndo} disabled={!props.undoAvailable || props.running} title={t("Undo the last turn and its worktree changes", "撤销最后一轮及其工作树修改")} aria-label={t("Undo last turn", "撤销最后一轮")}><Undo2 size={14} /></Button>
        <div className="composer-mode">
          <Select
            value={props.mode}
            options={runModeOptions}
            disabled={props.running}
            ariaLabel={t("Run mode", "运行模式")}
            menuPreferredWidth={240}
            menuMinimumWidth={200}
            menuAlign="right"
            onChange={props.onModeChange}
          />
        </div>
        <button type="button" className={`composer-rail-button composer-activity-button${runtimeActivity.runningTasks > 0 ? " is-active" : ""}`} onClick={() => window.dispatchEvent(new Event("sai:open-tasks"))} title={runtimeActivity.runningTasks > 0 ? t(`${runtimeActivity.runningTasks} background tasks running`, `${runtimeActivity.runningTasks} 个后台任务进行中`) : t("Open background tasks", "打开后台任务")} aria-label={t("Open background tasks", "打开后台任务")}>
          <Activity size={14} />
          {runtimeActivity.runningTasks > 0 && <span className="composer-activity-badge">{runtimeActivity.runningTasks}</span>}
        </button>
        {runtimeActivity.runningSubagents > 0 && (
          <button type="button" className="composer-rail-button composer-activity-button is-active" onClick={() => window.dispatchEvent(new Event("sai:open-subagents"))} title={t(`${runtimeActivity.runningSubagents} subagents running`, `${runtimeActivity.runningSubagents} 个子智能体运行中`)} aria-label={t("View subagents", "查看子智能体")}>
            <Bot size={14} />
            <span className="composer-activity-badge">{runtimeActivity.runningSubagents}</span>
          </button>
        )}
        <TodoMarkdownView sessionId={props.sessionId} compact />
      </div>
      <form className="composer" onSubmit={handleSubmit}>
        <AttachmentStrip attachments={props.attachments} onRemove={props.onRemoveAttachment} />
        <ComposerTextarea
          value={props.value}
          historyEntries={props.historyEntries}
          disabled={availability.inputDisabled}
          placeholder={props.sessionAvailable ? t("Type a message; press Enter to send", "输入消息，Enter 发送") : t("Select a session first", "请先选择会话")}
          onChange={props.onChange}
          onPasteImages={props.onAddImages}
          onSubmit={() => {
            if (!availability.sendDisabled) props.onSubmit();
          }}
        />
        <div className="composer-footer">
          <div className="composer-toolrail">
            <div className="composer-model-group">
              <ModelThinkingSelector
                choices={props.choices}
                selection={props.selection}
                thinkingLevel={props.thinkingLevel}
                loading={props.modelLoading}
                disabled={props.running}
                onModelSelect={props.onModelSelect}
                onThinkingLevelChange={props.onThinkingLevelChange}
              />
            </div>
          </div>
          <div className="composer-actions">
            <input ref={fileInputRef} type="file" accept="image/*" multiple onChange={handleFileChange} hidden />
            <button type="button" className="composer-icon-button" onClick={() => fileInputRef.current?.click()} disabled={availability.inputDisabled} aria-label={t("Add images", "添加图片")}><Paperclip size={18} /></button>
            {availability.showStop ? (
              <button type="button" className="composer-send stop" onClick={props.onStop} aria-label={t("Stop run", "停止运行")}><Square size={13} fill="currentColor" /></button>
            ) : (
              <button type="submit" className="composer-send" disabled={availability.sendDisabled} aria-label={t("Send message", "发送消息")}><ArrowRight size={18} /></button>
            )}
          </div>
        </div>
      </form>
    </div>
  );
}
