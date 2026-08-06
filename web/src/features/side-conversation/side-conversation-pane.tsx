import { ArrowRight, MessageSquareText, Square } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import { LiveRunMessage } from "../chat/chat-message";
import { ComposerSurface } from "../chat/composer/composer-surface";
import { useRunStream } from "../chat/use-run-stream";
import { useI18n } from "../i18n/use-i18n";
import { composeSideConversationInput } from "./side-conversation-context";
import { SIDE_CONVERSATION_SESSION_PREFIX, type SideConversationRequest } from "./side-conversation-events";
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
  const [error, setError] = useState<string | null>(null);
  const sessionRef = useRef<string | undefined>(undefined);
  const activeRunRef = useRef<string | undefined>(undefined);
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
   * @param event 表单提交事件
   * @returns 提交完成后的 Promise
   */
  const submit = async () => {
    const question = input.trim();
    if (!question || activeRun) return;
    setError(null);
    setInput("");
    try {
      const targetSessionId = await ensureSession();
      const modelInput = contextSent
        ? question
        : composeSideConversationInput(request.context, question);
      await run.start(
        targetSessionId,
        modelInput,
        request.mode,
        request.selection,
        undefined,
        request.thinkingLevel,
        request.agentId,
        question
      );
      setContextSent(true);
    } catch (cause) {
      setInput(question);
      setError(toDisplayError(cause, "Failed to start side conversation", "旁路对话启动失败").message);
    }
  };

  return (
    <section className="side-conversation-pane">
      <header className="side-conversation-head">
        <MessageSquareText size={15} aria-hidden />
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
      <ComposerSurface
        variant="compact"
        className="composer side-conversation-composer"
        value={input}
        historyEntries={[]}
        disabled={Boolean(activeRun)}
        submitDisabled={!input.trim() || Boolean(activeRun)}
        placeholder={t("Ask a question about this response", "针对这条回复提问")}
        onChange={setInput}
        onPasteImages={async () => undefined}
        onSubmit={() => void submit()}
      >
        <div className="composer-footer">
          <div className="composer-toolrail" />
          <div className="composer-actions">
            {activeRun ? (
              <button type="button" className="composer-send stop" onClick={() => activeRun.runId && void run.stop(activeRun.runId)} aria-label={t("Stop", "停止")} title={t("Stop", "停止")}>
                <Square size={12} fill="currentColor" />
              </button>
            ) : (
              <button className="composer-send" type="submit" disabled={!input.trim()} aria-label={t("Send", "发送")} title={t("Send", "发送")}>
                <ArrowRight size={16} />
              </button>
            )}
          </div>
        </div>
      </ComposerSurface>
    </section>
  );
}
