import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../api/client";
import type { RunMode } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { useChatAgentContext } from "../agents/chat-agent-context";
import { ChatComposer } from "./chat-composer";
import { HistoryTurn, LiveRunMessage } from "./chat-message";
import { projectConversationDisplay, retryableTurnId } from "./conversation-display";
import { MessageOverviewRail } from "./message-overview-rail";
import { createLiveOverviewItem, createTimelineOverviewItems } from "./message-overview-utils";
import { clearComposerDraft, readComposerDraft, writeComposerDraft } from "./composer-draft";
import { useComposerAttachments } from "./composer/use-composer-attachments";
import { useChatModel } from "./use-chat-model";
import { useRunStream } from "./use-run-stream";
import { useThinkingLevel } from "./use-thinking-level";
import { useFollowOutputScroll } from "./use-follow-output-scroll";
import "./chat-page.css";
import { ContextCompactionPart } from "./message/context-compaction-part";

/**
 * 渲染当前会话历史、实时运行事件和消息输入区。
 *
 * @returns 聊天页面
 */
export function ChatPage() {
  const queryClient = useQueryClient();
  const [input, setInput] = useState("");
  const [undoError, setUndoError] = useState<string | null>(null);
  const [undoConfirmOpen, setUndoConfirmOpen] = useState(false);
  const [actionBusy, setActionBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: api.sessions.list });
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list });
  const activeSession = sessions.data?.find((session) => session.active);
  const timeline = useQuery({
    queryKey: ["timeline", activeSession?.id],
    queryFn: () => api.sessions.timeline(activeSession!.id),
    enabled: Boolean(activeSession)
  });
  const onSettled = useCallback(() => {
    void Promise.all([
      activeSession?.id
        ? queryClient.invalidateQueries({ queryKey: ["timeline", activeSession.id] })
        : Promise.resolve(),
      queryClient.invalidateQueries({ queryKey: ["sessions"] }),
      queryClient.invalidateQueries({ queryKey: ["todos"] }),
      queryClient.invalidateQueries({ queryKey: ["system-usage"] })
    ]);
  }, [activeSession?.id, queryClient]);
  const onWorkspaceChanged = useCallback(() => {
    void Promise.all([
      queryClient.invalidateQueries({ queryKey: ["file-tree"] }),
      queryClient.invalidateQueries({ queryKey: ["file"] }),
      queryClient.invalidateQueries({ queryKey: ["workspace-diff"] })
    ]);
  }, [queryClient]);
  const onInterruptedWithoutReply = useCallback((restoredInput: string) => {
    setInput(restoredInput);
  }, []);
  const run = useRunStream(
    workspaces.data?.active_id,
    activeSession?.id,
    onSettled,
    onWorkspaceChanged,
    onInterruptedWithoutReply
  );
  const chatModel = useChatModel(activeSession?.id);
  const chatAgent = useChatAgentContext();
  const thinking = useThinkingLevel(activeSession?.id);
  const [mode, setMode] = useState<RunMode>("yolo");
  const composerAttachments = useComposerAttachments();
  const scrollRef = useRef<HTMLDivElement>(null);
  const display = useMemo(
    () => projectConversationDisplay(timeline.data?.turns ?? [], run.states),
    [timeline.data?.turns, run.states]
  );
  const scrollContentSignal = useMemo(
    () => [display.historyTurns, display.liveRuns],
    [display.historyTurns, display.liveRuns]
  );
  const { showJump, jumpToBottom, pauseFollowing } = useFollowOutputScroll(scrollRef, scrollContentSignal, activeSession?.id);

  // 切换会话时恢复该会话草稿；路由离开再回来也保留（模块级草稿缓存）。
  useEffect(() => {
    run.reset();
    setInput(readComposerDraft(activeSession?.id));
    composerAttachments.clearAttachments();
  }, [activeSession?.id]);

  // 输入变化时写入草稿，避免跳转设置/网关后丢失；同时把消息区滚到底部，方便看到最新上下文。
  useEffect(() => {
    writeComposerDraft(activeSession?.id, input);
  }, [activeSession?.id, input]);

  useEffect(() => {
    if (!input) return;
    jumpToBottom();
  }, [input, jumpToBottom]);

  /**
   * 提交前把输入中的 `/skill` 引用展开为完整 skill 文档。
   *
   * 输入框草稿仍保留短引用；仅发送给模型的文本会注入完整内容。
   *
   * @param value 用户当前输入
   * @returns 展开 skill 后的模型输入
   */
  const expandSkillsForSubmit = async (value: string): Promise<string> => {
    const { collectSkillMentionNames, expandSkillMentions } = await import("./composer/skill-mention-token");
    const names = collectSkillMentionNames(value);
    if (names.length === 0) return value;
    const documents: Record<string, string> = {};
    await Promise.all(names.map(async (name) => {
      try {
        const document = await api.skills.document(name);
        documents[name] = document.content;
      } catch {
        // 找不到或加载失败时保留原始 `/name` token
      }
    }));
    return expandSkillMentions(value, documents);
  };

  /** 提交当前输入内容和模型选择。 */
  const submit = async () => {
    const value = input.trim();
    if ((!value && composerAttachments.attachments.length === 0) || !activeSession) return;
    await queryClient.invalidateQueries({ queryKey: ["timeline", activeSession.id] });
    const originalInput = input;
    const currentAttachments = composerAttachments.attachments;
    const expanded = value ? await expandSkillsForSubmit(value) : value;
    setInput("");
    clearComposerDraft(activeSession.id);
    composerAttachments.clearAttachments();
    jumpToBottom();
    await run.start(
      activeSession.id,
      expanded,
      mode,
      chatModel.selection ?? undefined,
      currentAttachments.map((attachment) => attachment.dataUrl),
      thinking.thinkingLevel,
      chatAgent.selection?.id
    ).catch((error: unknown) => {
      setInput(originalInput);
      writeComposerDraft(activeSession.id, originalInput);
      composerAttachments.restoreAttachments(currentAttachments);
      throw error;
    });
  };

  const runningStates = run.states.filter((state) => !state.completed);
  const activeRun = runningStates.find((state) => state.status !== "queued") ?? runningStates[0];
  const running = runningStates.length > 0;
  const historyEntries = timeline.data?.turns.map((turn) => turn.user.content) ?? [];

  /**
   * 撤销最后一轮对话及该轮造成的工作树修改，并恢复用户输入。
   *
   * @returns 撤销完成后的 Promise
   */
  const undo = async () => {
    if (!activeSession || running) return;
    setUndoError(null);
    setUndoConfirmOpen(false);
    try {
      const outcome = await api.sessions.undo(activeSession.id);
      setInput(outcome.prompt ?? "");
      run.reset();
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["timeline", activeSession.id] }),
        queryClient.invalidateQueries({ queryKey: ["sessions"] }),
        queryClient.invalidateQueries({ queryKey: ["file-tree"] }),
        queryClient.invalidateQueries({ queryKey: ["file"] }),
        queryClient.invalidateQueries({ queryKey: ["workspace-diff"] })
      ]);
    } catch (error) {
      setUndoError(error instanceof Error ? error.message : String(error));
    }
  };
  const overviewItems = useMemo(
    () => [
      ...createTimelineOverviewItems(display.historyTurns),
      ...display.liveRuns.map(createLiveOverviewItem).filter((item) => item !== null)
    ],
    [display.historyTurns, display.liveRuns]
  );

  /** 回滚被点击的持久化轮次，并使用原输入重新发起运行。 */
  const retry = async (content: string, liveImages: string[] | undefined, candidateTurnId: string | null) => {
    if (!activeSession || running) return;
    if (!content.trim() && !(liveImages && liveImages.length > 0)) return;
    try {
      // 1. 主动读取最新时间线，避免终态事件和后台刷新之间的竞态
      const refreshedTimeline = await api.sessions.timeline(activeSession.id);
      queryClient.setQueryData(["timeline", activeSession.id], refreshedTimeline);
      const turnId = retryableTurnId(refreshedTimeline.turns, candidateTurnId);
      // 2. 已持久化的旧轮先从上下文删除，工具产生的工作树副作用保持不变
      if (turnId) await api.sessions.rollback(activeSession.id, turnId);
      // 3. 清理旧实时投影，避免旧轮和新轮同时渲染相同用户消息
      run.reset();
      await queryClient.invalidateQueries({ queryKey: ["timeline", activeSession.id] });
      // 4. 复用当前模式、模型与思考等级重新提交
      await run.start(activeSession.id, content, mode, chatModel.selection ?? undefined, liveImages, thinking.thinkingLevel, chatAgent.selection?.id);
    } catch (error) {
      setInput(content);
      throw error;
    }
  };
  const lastTurnId = timeline.data?.turns.at(-1)?.turn_id;
  const emptySession = !timeline.isLoading && display.historyTurns.length === 0 && display.liveRuns.length === 0;

  const forkFromTurn = async (turnId: string) => {
    if (!activeSession || actionBusy) return;
    setActionBusy(true);
    setActionError(null);
    try {
      // fork 后端已把新会话设为当前；再 switch 一次保证前端状态一致
      const session = await api.sessions.fork(activeSession.id, turnId);
      await api.sessions.switch(session.id);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["sessions"] }),
        queryClient.invalidateQueries({ queryKey: ["session-tree"] }),
        queryClient.invalidateQueries({ queryKey: ["timeline"] }),
        queryClient.invalidateQueries({ queryKey: ["timeline", session.id] })
      ]);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    } finally {
      setActionBusy(false);
    }
  };


  return (
    <div className={emptySession ? "chat-page empty-session" : "chat-page"}>
      <header className="chat-header">
        <h1>{activeSession?.title ?? "选择会话"}</h1>
      </header>
      <div className="message-scroll-region">
        <div className="message-scroll" ref={scrollRef}>
          <div className="message-column">
            {timeline.isLoading && <div className="empty-chat">正在读取会话历史</div>}
            {timeline.data?.compaction && !run.states.some((state) =>
              state.parts.some((part) => part.type === "compaction" && part.applied && part.summary)
            ) && (
              <div className="conversation-compaction" data-overview-id="history-compaction">
                <ContextCompactionPart
                  part={{
                    id: "history-compaction",
                    type: "compaction",
                    status: "completed",
                    turnCount: timeline.data.compaction.turn_count,
                    applied: timeline.data.compaction.applied,
                    summary: timeline.data.compaction.summary
                  }}
                />
              </div>
            )}
            {display.historyTurns.map((turn) => (
              <section className="conversation-turn" data-overview-id={`turn-${turn.turn_id}`} key={turn.turn_id}>
                <HistoryTurn
                  turn={turn}
                  onRetry={turn.turn_id === lastTurnId && !running
                    ? () => void retry(turn.user.content, undefined, turn.turn_id)
                    : undefined}
                  onFork={() => void forkFromTurn(turn.turn_id)}
                  actionBusy={actionBusy}
                />
              </section>
            ))}
            {display.liveRuns.map((state) => (
              <section className="conversation-turn" data-overview-id={`live-${state.runId}`} key={state.runId}>
                <LiveRunMessage
                  state={state}
                  running={!state.completed}
                  onRetry={!running && state.completed
                    ? () => void retry(state.userInput, state.imageUrls, state.runId)
                    : undefined}
                />
              </section>
            ))}
            {timeline.error && <div className="run-error">{timeline.error.message}</div>}
            {chatModel.error && <div className="run-error">{chatModel.error.message}</div>}
            {actionError && <div className="run-error">{actionError}</div>}
          </div>
        </div>
        <MessageOverviewRail
          scrollContainerRef={scrollRef}
          items={overviewItems}
          onNavigate={pauseFollowing}
        />
        {showJump && (
          <button type="button" className="jump-to-bottom" onClick={jumpToBottom} aria-label="回到底部" title="回到底部">
            <ArrowDown size={16} />
          </button>
        )}
      </div>
      {emptySession && (
        <div className="empty-session-greeting">
          <h2>开始新的对话</h2>
          <p>输入任务或问题，Enter 发送，Shift+Enter 换行</p>
        </div>
      )}
      <ChatComposer
        value={input}
        mode={mode}
        attachments={composerAttachments.attachments}
        historyEntries={historyEntries}
        thinkingLevel={thinking.thinkingLevel}
        choices={chatModel.choices}
        selection={chatModel.selection}
        modelLoading={chatModel.isLoading}
        running={running}
        runStatus={activeRun?.status ?? "idle"}
        sessionAvailable={Boolean(activeSession)}
        undoAvailable={Boolean(timeline.data?.turns.length)}
        agentChoices={chatAgent.choices}
        agentSelection={chatAgent.selection}
        agentLoading={chatAgent.isLoading}
        sessionId={activeSession?.id}
        onChange={setInput}
        onModeChange={setMode}
        onThinkingLevelChange={thinking.setThinkingLevel}
        onAddImages={composerAttachments.addFiles}
        onRemoveAttachment={composerAttachments.removeAttachment}
        onModelSelect={chatModel.selectModel}
        onSubmit={() => void submit()}
        onStop={() => activeRun?.runId && void run.stop(activeRun.runId)}
        onUndo={() => setUndoConfirmOpen(true)}
        onAgentSelect={chatAgent.selectAgent}
        onCompact={() => activeSession
          ? run.startCompaction(activeSession.id, chatModel.selection ?? undefined)
          : Promise.resolve()}
      />
      <Modal
        open={undoConfirmOpen}
        title="撤销上一轮？"
        description="将删除最后一轮对话，并尝试回滚该轮对工作树的修改；用户输入会恢复到输入框。"
        size="small"
        onClose={() => setUndoConfirmOpen(false)}
        footer={(
          <>
            <Button onClick={() => setUndoConfirmOpen(false)}>取消</Button>
            <Button variant="danger" onClick={() => void undo()}>确认撤销</Button>
          </>
        )}
      >
        <p>此操作不可通过同一按钮再次恢复。若工作树在本轮后继续被改动，撤销可能失败。</p>
      </Modal>
      <Modal
        open={Boolean(undoError)}
        title="撤销失败"
        description="工作树在本轮结束后又发生变化，因此没有执行可能覆盖新修改的撤销。"
        size="small"
        onClose={() => setUndoError(null)}
        footer={<Button onClick={() => setUndoError(null)}>关闭</Button>}
      >
        <p>{undoError}</p>
      </Modal>
    </div>
  );
}
