import { MessageSquareText } from "../../shared/ui/icons";
import { useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { RunMode, RunModelSelection } from "../../api/contracts";
import { LiveRunMessage } from "../chat/chat-message";
import { useChatModel } from "../chat/use-chat-model";
import { useRunStream } from "../chat/use-run-stream";
import { useComposerAttachments } from "../chat/composer/use-composer-attachments";
import { useI18n } from "../i18n/use-i18n";
import { composeSideConversationInput } from "./side-conversation-context";
import { SIDE_CONVERSATION_SESSION_PREFIX, type SideConversationRequest } from "./side-conversation-events";
import { SideConversationComposer } from "./side-conversation-composer";
import "./side-conversation-pane.css";

type SideConversationPaneProps = {
  request: SideConversationRequest;
};

/**
 * 渲染与主会话隔离的临时问答面板。
 *
 * @param props 已冻结的主会话上下文与运行偏好
 * @returns 旁路对话界面
 */
export function SideConversationPane({ request }: SideConversationPaneProps) {
  const { t } = useI18n();
  const [input, setInput] = useState("");
  const [sessionId, setSessionId] = useState<string>();
  const [contextSent, setContextSent] = useState(false);
  const [mode, setMode] = useState<RunMode>(request.mode);
  const [thinkingLevel, setThinkingLevel] = useState(request.thinkingLevel);
  const [modelSelection, setModelSelection] = useState<RunModelSelection | null>(request.selection ?? null);
  const [error, setError] = useState<string | null>(null);
  const composerAttachments = useComposerAttachments(`side:${request.id}`);
  const sessionRef = useRef<string | undefined>(undefined);
  const activeRunRef = useRef<string | undefined>(undefined);
  const submittingRef = useRef(false);
  const [submitting, setSubmitting] = useState(false);
  const modelPreferences = useChatModel(`side:${request.id}`);
  const effectiveModelSelection = modelSelection ?? modelPreferences.selection;
  const selectedModel = effectiveModelSelection
    ? modelPreferences.choices.find((choice) => choice.providerId === effectiveModelSelection.providerId && choice.model === effectiveModelSelection.model) ?? null
    : null;
  const run = useRunStream(request.workspaceId, sessionId, () => undefined);
  const activeRun = run.states.find((state) => !state.completed);

  useEffect(() => {
    activeRunRef.current = activeRun?.runId ?? undefined;
  }, [activeRun?.runId]);

  useEffect(() => {
    return () => {
      const temporarySessionId = sessionRef.current;
      if (!temporarySessionId) return;
      const removeTemporarySession = () => api.sessions.remove(temporarySessionId).catch(() => undefined);
      const activeRunId = activeRunRef.current;
      if (activeRunId) {
        void api.runs.stop(activeRunId).catch(() => undefined).then(removeTemporarySession);
      } else {
        void removeTemporarySession();
      }
    };
  }, []);

  /**
   * 创建内部临时会话，并立即恢复主会话的活动状态。
   *
   * @returns 可供旁路运行使用的临时会话标识
   */
  const ensureSession = async (): Promise<string> => {
    if (sessionRef.current) return sessionRef.current;
    const created = await api.sessions.create(`${SIDE_CONVERSATION_SESSION_PREFIX}${request.title}`, request.workspaceId);
    try {
      await api.sessions.switch(request.sourceSessionId);
    } catch (cause) {
      await api.sessions.remove(created.id).catch(() => undefined);
      throw cause;
    }
    sessionRef.current = created.id;
    setSessionId(created.id);
    return created.id;
  };

  /**
   * 提交旁路问题；只有首轮附带冻结上下文。
   *
   * @returns 提交完成后的 Promise
   */
  const submit = async () => {
    const question = input.trim();
    const attachments = composerAttachments.attachments;
    if ((!question && attachments.length === 0) || activeRun || submittingRef.current) return;
    submittingRef.current = true;
    setSubmitting(true);
    setError(null);
    setInput("");
    try {
      const targetSessionId = await ensureSession();
      const modelInput = contextSent
        ? question
        : composeSideConversationInput(request.context, question);
      const imageUrls = attachments.map((attachment) => attachment.dataUrl);
      await run.start(
        targetSessionId,
        modelInput,
        mode,
        effectiveModelSelection ?? undefined,
        imageUrls,
        thinkingLevel,
        request.agentId,
        question
      );
      composerAttachments.clearAttachments();
      setContextSent(true);
    } catch (cause) {
      setInput(question);
      setError(toDisplayError(cause, "Failed to start side conversation", "旁路对话启动失败").message);
    } finally {
      submittingRef.current = false;
      setSubmitting(false);
    }
  };

  /**
   * 统一处理文件选择、粘贴及拖入图片的失败反馈。
   * @param files 图片文件；start 和 end 为编辑器选区
   * @returns 添加成功后的光标位置，失败时返回 undefined
   */
  const addImages = async (files: File[], start: number, end: number): Promise<number | undefined> => {
    try {
      return await composerAttachments.addFiles(files, start, end);
    } catch (cause) {
      setError(toDisplayError(cause, "Failed to add image", "添加图片失败").message);
      return undefined;
    }
  };

  return (
    <section className="side-conversation-pane">
      <header className="side-conversation-head">
        <MessageSquareText size={16} aria-hidden />
        <div>
          <strong>{request.title}</strong>
          <span>{t("From the main conversation", "来自主会话")}</span>
        </div>
      </header>
      <div className="side-conversation-messages">
        {run.states.length === 0 && (
          <div className="side-conversation-empty">
            <MessageSquareText size={20} aria-hidden />
            <span>{t("Ask about the selected response", "针对所选回复提出疑问")}</span>
          </div>
        )}
        {run.states.map((state) => (
          <LiveRunMessage key={state.runId} state={state} sessionId={sessionId} running={!state.completed} />
        ))}
        {error && <div className="side-conversation-error" role="alert">{error}</div>}
      </div>
      <SideConversationComposer
        value={input} attachments={composerAttachments.attachments}
        running={Boolean(activeRun)} submitting={submitting} mode={mode} onModeChange={setMode}
        onChange={setInput} onPasteImages={addImages} onRemoveAttachment={composerAttachments.removeAttachment}
        onSubmit={() => void submit()} onStop={() => { if (activeRun?.runId) void run.stop(activeRun.runId); }}
        model={{
          choices: modelPreferences.choices, selection: selectedModel, thinkingLevel,
          thinkingLevels: modelPreferences.thinkingLevels, loading: modelPreferences.isLoading,
          disabled: Boolean(activeRun) || submitting, onModelSelect: setModelSelection,
          onThinkingLevelChange: setThinkingLevel
        }}
      />
    </section>
  );
}
