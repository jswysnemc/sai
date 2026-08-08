import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, GitBranch } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { RunMode } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { HoverRevealButton } from "../../shared/ui/hover-reveal-button/hover-reveal-button";
import { SkeletonText } from "../../shared/ui/skeleton/skeleton";
import { Modal } from "../../shared/ui/dialog/modal";
import { useChatAgentContext } from "../agents/chat-agent-context";
import { ChatComposer } from "./chat-composer";
import { ChatSessionHeader } from "./chat-session-header";
import { HistoryTurn, LiveRunMessage } from "./chat-message";
import { projectConversationDisplay } from "./conversation-display";
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
import { ContextPromptBanner } from "./message/context-prompt-banner";
import { errorDetailForDisplay, RunErrorNotice } from "./message/run-error-notice";
import { useI18n } from "../i18n/use-i18n";
import { parseGoalCommand } from "../goals/goal-command";
import { appendTerminalSelection, FOCUS_COMPOSER_EVENT, INSERT_TERMINAL_SELECTION_EVENT, type TerminalSelectionDetail } from "./composer/composer-events";
import { RuntimeOverview } from "../runtime-overview/runtime-overview";
import { BranchSwitcher } from "./turn-tree/branch-switcher";
import { TurnTreeOverview } from "./turn-tree/turn-tree-overview";
import { TurnTreePanel } from "./turn-tree/turn-tree-panel";
import { useBranchActions } from "./turn-tree/use-branch-actions";
import { useResendActions } from "./turn-tree/use-resend-actions";
import { useTurnTree } from "./turn-tree/use-turn-tree";
import { QueuedMessageList } from "./queue/queued-message-list";
import { isConversationEmpty, shouldCenterEmptySession } from "./empty-session-layout";
import { createSideConversationRequest } from "../side-conversation/side-conversation-context";
import { openSideConversation } from "../side-conversation/side-conversation-events";

/**
 * 渲染当前会话历史、实时运行事件和消息输入区。
 *
 * @returns 聊天页面
 */
export function ChatPage() {
  const { locale, t } = useI18n();
  const queryClient = useQueryClient();
  const [input, setInput] = useState("");
  const [undoError, setUndoError] = useState<Error | null>(null);
  const [undoConfirmOpen, setUndoConfirmOpen] = useState(false);
  const [actionBusy, setActionBusy] = useState(false);
  const [actionError, setActionError] = useState<Error | null>(null);
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: api.sessions.list });
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list });
  const gitStatus = useQuery({
    queryKey: ["runtime-overview", "git-status"],
    queryFn: () => api.workspace.gitStatus(),
    refetchInterval: 2500,
    retry: false
  });
  const activeSession = sessions.data?.find((session) => session.active);
  const [treeOpen, setTreeOpen] = useState(false);
  const [treeOverviewOpen, setTreeOverviewOpen] = useState(false);
  const activeWorkspace = workspaces.data?.workspaces.find(
    (workspace) => workspace.id === workspaces.data.active_id
  );
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
      activeSession?.id
        ? queryClient.invalidateQueries({ queryKey: ["session-turn-tree", activeSession.id] })
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
  const turnTree = useTurnTree(activeSession?.id, {
    onBranchChanged: run.reset,
    onError: (error) => setActionError(toDisplayError(
      error,
      "Failed to switch the conversation branch",
      "切换会话分支失败"
    ))
  });
  const chatModel = useChatModel(activeSession?.id);
  const chatAgent = useChatAgentContext();
  const thinking = useThinkingLevel(activeSession?.id);
  const [mode, setMode] = useState<RunMode>("yolo");
  const composerAttachments = useComposerAttachments(activeSession?.id);
  const scrollRef = useRef<HTMLDivElement>(null);
  const display = useMemo(
    () => projectConversationDisplay(timeline.data?.turns ?? [], run.states, activeSession?.id),
    [activeSession?.id, timeline.data?.turns, run.states]
  );
  const [submittedEmptySessionId, setSubmittedEmptySessionId] = useState<string | null>(null);
  const activeLiveRuns = useMemo(
    () => display.liveRuns.filter((state) => state.status !== "queued"),
    [display.liveRuns]
  );
  const queuedRuns = useMemo(
    () => display.liveRuns.filter((state) => state.status === "queued"),
    [display.liveRuns]
  );
  const hasHistoryCompaction = Boolean(timeline.data?.compaction?.summary?.trim());
  const conversationEmpty = isConversationEmpty({
    timelineLoading: timeline.isLoading,
    historyTurnCount: display.historyTurns.length,
    liveRunCount: display.liveRuns.length,
    hasHistoryCompaction
  });
  const centerEmptySession = shouldCenterEmptySession(
    conversationEmpty,
    activeSession?.id,
    submittedEmptySessionId
  );
  const scrollContentSignal = useMemo(
    () => [display.historyTurns, activeLiveRuns, queuedRuns],
    [activeLiveRuns, display.historyTurns, queuedRuns]
  );
  const { showJump, jumpToBottom, pauseFollowing } = useFollowOutputScroll(scrollRef, scrollContentSignal, activeSession?.id);

  // 切换会话时恢复该会话草稿；路由离开再回来也保留（模块级草稿缓存）。
  useEffect(() => {
    run.reset();
    setInput(readComposerDraft(activeSession?.id));
    setSubmittedEmptySessionId(null);
  }, [activeSession?.id]);

  // 首条消息进入时间线或实时状态后清理乐观布局标记
  useEffect(() => {
    if (conversationEmpty) return;
    setSubmittedEmptySessionId((current) => current === activeSession?.id ? null : current);
  }, [activeSession?.id, conversationEmpty]);

  // 输入变化时写入草稿，避免跳转设置/网关后丢失；同时把消息区滚到底部，方便看到最新上下文。
  useEffect(() => {
    writeComposerDraft(activeSession?.id, input);
  }, [activeSession?.id, input]);

  useEffect(() => {
    if (!input) return;
    jumpToBottom();
  }, [input, jumpToBottom]);

  useEffect(() => {
    /** 将终端右键菜单发送的选区追加为输入原子。 */
    const handleTerminalSelection = (event: Event) => {
      if (!activeSession) return;
      const detail = (event as CustomEvent<TerminalSelectionDetail>).detail;
      if (!detail?.content) return;
      setInput((current) => appendTerminalSelection(current, detail));
      jumpToBottom();
    };
    window.addEventListener(INSERT_TERMINAL_SELECTION_EVENT, handleTerminalSelection);
    return () => window.removeEventListener(INSERT_TERMINAL_SELECTION_EVENT, handleTerminalSelection);
  }, [activeSession, jumpToBottom]);

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
    if (turnTree.switchBranch.isPending || branchActions.pending) return;
    const value = input.trim();
    if ((!value && composerAttachments.attachments.length === 0) || !activeSession) return;
    const goalCommand = parseGoalCommand(value);
    if (goalCommand) {
      if (!goalCommand.objective && composerAttachments.attachments.length === 0) {
        setActionError(new Error(t("Enter an objective after /goal", "请在 /goal 后输入目标内容")));
        return;
      }
      const originalInput = input;
      const currentAttachments = composerAttachments.attachments;
      if (conversationEmpty) setSubmittedEmptySessionId(activeSession.id);
      // 1. 保留 skill-mention / 文件原子，并把当前附件图片并入目标正文供展开查看
      const objectiveWithMedia = [
        goalCommand.objective.trim(),
        ...currentAttachments.map((attachment, index) => `![goal-image-${index + 1}](${attachment.dataUrl})`)
      ].filter(Boolean).join("\n\n");
      setActionError(null);
      setInput("");
      clearComposerDraft(activeSession.id);
      composerAttachments.clearAttachments();
      jumpToBottom();
      try {
        // 2. 将命令后的完整输入（含技能标记与图片）保存为当前会话目标
        const response = await api.goals.set(activeSession.id, objectiveWithMedia);
        queryClient.setQueryData(["goal", activeSession.id], response);
        // 3. 目标保存成功后立即启动主动续轮
        await run.startGoal(
          activeSession.id,
          mode,
          chatModel.selection ?? undefined,
          thinking.thinkingLevel,
          chatAgent.selection?.id,
          originalInput
        );
      } catch (error) {
        setSubmittedEmptySessionId((current) => current === activeSession.id ? null : current);
        setInput(originalInput);
        writeComposerDraft(activeSession.id, originalInput);
        composerAttachments.restoreAttachments(currentAttachments);
        setActionError(toDisplayError(error, "Failed to start goal", "启动目标失败"));
      }
      return;
    }
    const originalInput = input;
    const currentAttachments = composerAttachments.attachments;
    if (conversationEmpty) setSubmittedEmptySessionId(activeSession.id);
    try {
      await queryClient.invalidateQueries({ queryKey: ["timeline", activeSession.id] });
      const expanded = value ? await expandSkillsForSubmit(value) : value;
      setActionError(null);
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
      );
      // 记录原始输入而非技能展开后的文本：上下键翻回来的应当是用户当初打的内容
      if (value) {
        void api.inputHistory
          .append(value)
          .then((response) => queryClient.setQueryData(["input-history"], response))
          .catch(() => undefined);
      }
    } catch (error) {
      setSubmittedEmptySessionId((current) => current === activeSession.id ? null : current);
      setInput(originalInput);
      writeComposerDraft(activeSession.id, originalInput);
      composerAttachments.restoreAttachments(currentAttachments);
      setActionError(toDisplayError(error, "Failed to start the run", "启动运行失败"));
    }
  };

  /**
   * 校验并加入图片附件，失败时使用聊天页统一错误提示。
   *
   * @param files 待加入图片文件
   * @param selectionStart 当前选区起点
   * @param selectionEnd 当前选区终点
   * @returns 更新后的光标位置；失败时返回 undefined
   */
  const addComposerImages = async (files: File[], selectionStart: number, selectionEnd: number) => {
    try {
      setActionError(null);
      return await composerAttachments.addFiles(files, selectionStart, selectionEnd);
    } catch (error) {
      setActionError(toDisplayError(error, "Failed to attach image", "添加图片失败"));
      return undefined;
    }
  };

  const runningStates = run.states.filter((state) => !state.completed);
  const activeRun = runningStates.find((state) => state.status !== "queued") ?? runningStates[0];
  const running = runningStates.length > 0;
  // 输入历史与 TUI 共用同一份跨会话存储，切换会话后仍可翻到之前输入过的内容
  const inputHistory = useQuery({
    queryKey: ["input-history"],
    queryFn: api.inputHistory.list,
    staleTime: 30_000
  });
  const historyEntries = inputHistory.data?.entries ?? [];
  // 基于会话树的分支动作：只移动活动叶子指针，不删除任何轮次
  const branchActions = useBranchActions({
    sessionId: activeSession?.id,
    running,
    resetRun: run.reset,
    onFocusComposer: () => window.dispatchEvent(new Event(FOCUS_COMPOSER_EVENT)),
    onError: (error, fallbackEn, fallbackZh) =>
      setActionError(toDisplayError(error, fallbackEn, fallbackZh))
  });
  const branchTransitioning = turnTree.switchBranch.isPending || branchActions.pending;

  /**
   * 撤销上一轮：把对话位置退回父轮次，不删除任何内容。
   *
   * 与旧的删除式撤销不同，退出的轮次仍留在树中，可以随时切回对比。
   *
   * @returns 撤销完成后的 Promise
   */
  const undoToPreviousTurn = async () => {
    const turnId = timeline.data?.turns.filter((turn) => !turn.automatic).at(-1)?.turn_id;
    if (!turnId) return;
    setUndoError(null);
    setUndoConfirmOpen(false);
    // 退回后把该轮的用户输入放回输入框，便于直接改写重发
    const prompt = timeline.data?.turns.find((turn) => turn.turn_id === turnId)?.user.content;
    await branchActions.undoToParent(turnId);
    setInput(prompt ?? "");
  };

  const overviewItems = useMemo(
    () => [
      ...createTimelineOverviewItems(display.historyTurns, undefined, locale),
      ...activeLiveRuns.map((state) => createLiveOverviewItem(state, locale)).filter((item) => item !== null)
    ],
    [activeLiveRuns, display.historyTurns, locale]
  );

  // 重试与编辑重发共用同一套分支语义，抽到 hook 内维护
  const resend = useResendActions({
    sessionId: activeSession?.id,
    running,
    mode,
    selection: chatModel.selection ?? undefined,
    thinkingLevel: thinking.thinkingLevel,
    agentId: chatAgent.selection?.id,
    moveToParent: branchActions.moveToParentForRetry,
    resetRun: run.reset,
    startRun: (content, imageUrls) => run.start(
      activeSession!.id,
      content,
      mode,
      chatModel.selection ?? undefined,
      imageUrls,
      thinking.thinkingLevel,
      chatAgent.selection?.id
    ),
    onError: (error, fallbackEn, fallbackZh) =>
      setActionError(toDisplayError(error, fallbackEn, fallbackZh))
  });

  /**
   * 以改写后的内容重发某一轮，期间禁用相关操作按钮。
   *
   * @param turnId 被编辑的轮次标识
   * @param content 改写后的正文
   * @param imageUrls 改写后的图片列表
   * @returns 重发完成的 Promise
   */
  const editAndResend = async (turnId: string | null, content: string, imageUrls: string[]) => {
    if (actionBusy) return;
    setActionBusy(true);
    try {
      await resend.editAndResend(turnId, content, imageUrls);
    } finally {
      setActionBusy(false);
    }
  };

  const lastTurnId = timeline.data?.turns.filter((turn) => !turn.automatic).at(-1)?.turn_id;

  /**
   * 以指定已完成助手回复及此前上下文打开旁路对话。
   *
   * @param turnId 作为旁路上下文终点的轮次标识
   * @returns 无返回值
   */
  const openTurnSideConversation = (turnId: string) => {
    if (!activeSession || !activeWorkspace) return;
    const request = createSideConversationRequest({
      turns: timeline.data?.turns ?? [],
      sourceTurnId: turnId,
      workspaceId: activeWorkspace.id,
      sourceSessionId: activeSession.id,
      mode,
      selection: chatModel.selection ?? undefined,
      thinkingLevel: thinking.thinkingLevel,
      agentId: chatAgent.selection?.id
    });
    if (request) openSideConversation(request);
  };

  const composer = (
    <ChatComposer
      value={input}
      mode={mode}
      attachments={composerAttachments.attachments}
      historyEntries={historyEntries}
      thinkingLevel={thinking.thinkingLevel}
      thinkingLevels={chatModel.thinkingLevels}
      choices={chatModel.choices}
      selection={chatModel.selection}
      modelLoading={chatModel.isLoading}
      running={running}
      submitBlocked={branchTransitioning}
      runStatus={activeRun?.status ?? "idle"}
      sessionAvailable={Boolean(activeSession)}
      undoAvailable={Boolean(timeline.data?.turns.length) && !branchTransitioning}
      agentChoices={chatAgent.choices}
      agentSelection={chatAgent.selection}
      agentLoading={chatAgent.isLoading}
      sessionId={activeSession?.id}
      onChange={setInput}
      onModeChange={setMode}
      onThinkingLevelChange={thinking.setThinkingLevel}
      onAddImages={addComposerImages}
      onRemoveAttachment={composerAttachments.removeAttachment}
      onModelSelect={chatModel.selectModel}
      onSubmit={() => void submit()}
      onStop={() => activeRun?.runId && void run.stop(activeRun.runId)}
      onUndo={() => setUndoConfirmOpen(true)}
      onAgentSelect={chatAgent.selectAgent}
      onCompact={() => activeSession
        ? run.startCompaction(activeSession.id, chatModel.selection ?? undefined)
        : Promise.resolve()}
      onContinueGoal={() => activeSession
        ? run.startGoal(
            activeSession.id,
            mode,
            chatModel.selection ?? undefined,
            thinking.thinkingLevel,
            chatAgent.selection?.id
          )
        : Promise.resolve()}
    />
  );

  return (
    <div className={centerEmptySession ? "chat-page empty-session" : "chat-page"}>
      <div className="message-scroll-region">
        <div className="message-scroll" ref={scrollRef}>
          <ChatSessionHeader
            title={activeSession?.title ?? t("Select a session", "选择会话")}
            workspace={activeWorkspace}
            branch={gitStatus.data?.status === "ready" ? gitStatus.data.head : undefined}
          />
          <div className="message-column">
            {(timeline.isLoading || branchTransitioning) && (
              <div className="chat-timeline-skeleton">
                <SkeletonText label={t("Loading conversation history", "正在读取会话历史")} lines={4} />
              </div>
            )}
            {activeSession && !timeline.isLoading && !branchTransitioning && !conversationEmpty && (
              <ContextPromptBanner
                sessionId={activeSession.id}
                agentId={chatAgent.selection?.id}
                mode={mode}
                selection={chatModel.selection}
              />
            )}
            {!branchTransitioning && timeline.data?.compaction && !run.states.some((state) =>
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
            {!branchTransitioning && display.historyTurns.map((turn) => (
              <section className="conversation-turn" data-overview-id={`turn-${turn.turn_id}`} key={turn.turn_id}>
                <HistoryTurn
                  turn={turn}
                  sessionId={activeSession?.id}
                  onRetry={turn.turn_id === lastTurnId && !running
                    ? () => void resend.retry(turn.user.content, turn.user.image_urls, turn.turn_id)
                    : undefined}
                  onContinueFrom={!running ? () => void branchActions.continueFrom(turn.turn_id) : undefined}
                  onSideConversation={turn.status === "completed" && turn.assistant.content.trim()
                    ? () => openTurnSideConversation(turn.turn_id)
                    : undefined}
                  onEditResend={!running
                    ? (content, imageUrls) => void editAndResend(turn.turn_id, content, imageUrls)
                    : undefined}
                  actionBusy={actionBusy || branchTransitioning}
                  branchSlot={(
                    <BranchSwitcher
                      tree={turnTree.tree.data}
                      turnId={turn.turn_id}
                      busy={turnTree.switchBranch.isPending || running}
                      onSwitch={(target) => turnTree.switchBranch.mutate(target)}
                    />
                  )}
                />
              </section>
            ))}
            {!branchTransitioning && activeLiveRuns.map((state) => (
              <section className="conversation-turn" data-overview-id={`live-${state.runId}`} key={state.runId}>
                <LiveRunMessage
                  state={state}
                  sessionId={activeSession?.id}
                  running={!state.completed}
                  onRetry={!running && state.completed
                    ? () => void resend.retry(state.userInput, state.imageUrls, state.runId)
                    : undefined}
                  onEditResend={!running && state.completed
                    ? (content, imageUrls) => void editAndResend(state.runId, content, imageUrls)
                    : undefined}
                  actionBusy={actionBusy || branchTransitioning}
                />
              </section>
            ))}
            <QueuedMessageList
              runs={queuedRuns}
              onUpdate={run.updateQueuedInput}
              onMove={run.moveQueuedRun}
              onRemove={run.removeQueuedRun}
              onError={(error) => setActionError(toDisplayError(error, "Failed to update the message queue", "更新消息队列失败"))}
            />
            {timeline.error && <RunErrorNotice message={timeline.error.message} detail={errorDetailForDisplay(timeline.error)} />}
            {chatModel.error && <RunErrorNotice message={chatModel.error.message} detail={errorDetailForDisplay(chatModel.error)} />}
            {actionError && <RunErrorNotice message={actionError.message} detail={errorDetailForDisplay(actionError)} />}
          </div>
          {centerEmptySession ? (
            <div className="empty-session-stage">
              <div className="empty-session-greeting">
                <h2>{t("Start a new conversation", "开始新的对话")}</h2>
                <p>{t("Enter a task or question. Press Enter to send and Shift+Enter for a new line.", "输入任务或问题，Enter 发送，Shift+Enter 换行")}</p>
              </div>
              {composer}
            </div>
          ) : (
            composer
          )}
        </div>
        <MessageOverviewRail
          scrollContainerRef={scrollRef}
          items={overviewItems}
          onNavigate={pauseFollowing}
        />
        {!treeOpen && (turnTree.tree.data?.total_turns ?? 0) > 0 && (
          <button
            type="button"
            className="turn-tree-open"
            onClick={() => setTreeOpen(true)}
            aria-label={t("Show session branches", "查看会话分支")}
            title={t("Show session branches", "查看会话分支")}
          >
            <GitBranch size={13} aria-hidden />
            <span className="turn-tree-open-label">{t("Branches", "会话分支")}</span>
            {(turnTree.tree.data?.branch_points ?? 0) > 0 && (
              <span className="turn-tree-open-count">{turnTree.tree.data?.branch_points}</span>
            )}
          </button>
        )}
        {treeOpen && turnTree.tree.data && (
          <TurnTreePanel
            tree={turnTree.tree.data}
            busy={turnTree.switchBranch.isPending || running}
            onSelect={(turnId) => turnTree.switchBranch.mutate(turnId)}
            onClose={() => setTreeOpen(false)}
            onOpenOverview={() => setTreeOverviewOpen(true)}
          />
        )}
        <RuntimeOverview sessionId={activeSession?.id} />
        {showJump && (
          <HoverRevealButton
            className="jump-to-bottom"
            icon={<ArrowDown size={16} />}
            label={t("Jump to bottom", "回到底部")}
            onClick={jumpToBottom}
          />
        )}
      </div>
      <Modal
        open={treeOverviewOpen}
        title={t("Branch overview", "分支总览")}
        description={t("Click any turn to switch the conversation to that branch.", "点击任意轮次即可把对话切换到该分支。")}
        size="large"
        className="turn-tree-overview-modal"
        onClose={() => setTreeOverviewOpen(false)}
      >
        {turnTree.tree.data && (
          <TurnTreeOverview
            tree={turnTree.tree.data}
            busy={turnTree.switchBranch.isPending || running}
            onSelect={(turnId) => {
              turnTree.switchBranch.mutate(turnId, {
                onSuccess: () => setTreeOverviewOpen(false)
              });
            }}
          />
        )}
      </Modal>
      <Modal
        open={undoConfirmOpen}
        title={t("Step back to the previous turn?", "退回上一轮？")}
        description={t("The conversation moves back one turn. Nothing is deleted — the turn you step out of stays in the branch tree and can be reached again.", "对话位置退回一轮。不会删除任何内容：退出的这一轮仍保留在分支树中，随时可以切回。")}
        size="small"
        onClose={() => setUndoConfirmOpen(false)}
        footer={(
          <>
            <Button onClick={() => setUndoConfirmOpen(false)}>{t("Cancel", "取消")}</Button>
            <Button onClick={() => void undoToPreviousTurn()}>{t("Step back", "确认退回")}</Button>
          </>
        )}
      >
        <p>{t("Worktree changes made by that turn are not rolled back. The user input returns to the composer so you can revise and resend.", "该轮对工作树的修改不会被回滚。用户输入会回到输入框，便于修改后重新发送。")}</p>
      </Modal>
      <Modal
        open={Boolean(undoError)}
        title={t("Undo failed", "撤销失败")}
        description={t("The worktree changed after the turn completed, so Sai did not run an undo that could overwrite newer changes.", "工作树在本轮结束后又发生变化，因此没有执行可能覆盖新修改的撤销。")}
        size="small"
        onClose={() => setUndoError(null)}
        footer={<Button onClick={() => setUndoError(null)}>{t("Close", "关闭")}</Button>}
      >
        <p>{undoError?.message}</p>
      </Modal>
    </div>
  );
}
