import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { RunMode } from "../../api/contracts";
import { HoverRevealButton } from "../../shared/ui/hover-reveal-button/hover-reveal-button";
import { SkeletonText } from "../../shared/ui/skeleton/skeleton";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { Toast, useToast } from "../../shared/ui/notify/notify";
import { useChatAgentContext } from "../agents/chat-agent-context";
import { TrajectoryView } from "../trajectory/trajectory-view";
import { collectChatErrorNotices } from "./chat-error-notices";
import { ChatConversation } from "./chat-conversation";
import { ChatViewSwitch } from "./chat-view-switch";
import { ChatSessionDialogs } from "./chat-session-dialogs";
import { ChatEmptyState } from "./chat-empty-state";
import { ChatComposer } from "./chat-composer";
import { ChatSessionHeader } from "./chat-session-header";
import { projectConversationDisplay } from "./conversation-display";
import { MessageOverviewRail } from "./message-overview-rail";
import { createLiveOverviewItem, createTimelineOverviewItems } from "./message-overview-utils";
import { clearToolExpandState } from "./message/tool-expand-state";
import { clearComposerDraft, readComposerDraft, writeComposerDraft } from "./composer-draft";
import { useComposerAttachments } from "./composer/use-composer-attachments";
import { useChatModel } from "./use-chat-model";
import { useRunStream } from "./use-run-stream";
import { useThinkingLevel } from "./use-thinking-level";
import { useFollowOutputScroll } from "./use-follow-output-scroll";
import "./chat-page.css";
import { ContextCompactionPart } from "./message/context-compaction-part";
import { ContextPromptBanner } from "./message/context-prompt-banner";
import { RunErrorNotice } from "./message/run-error-notice";
import { useI18n } from "../i18n/use-i18n";
import { parseGoalCommand } from "../goals/goal-command";
import { parseRenameCommand } from "../sessions/rename-command";
import { appendTerminalSelection, FOCUS_COMPOSER_EVENT, INSERT_TERMINAL_SELECTION_EVENT, type TerminalSelectionDetail } from "./composer/composer-events";
import { TurnTreePanel } from "./turn-tree/turn-tree-panel";
import { TurnTreeNavigation } from "./turn-tree/turn-tree-navigation";
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
export function ChatPage({ toolbar }: { toolbar?: ReactNode }) {
  const { locale, t } = useI18n();
  const confirm = useConfirm();
  const { notice, showToast, dismissToast } = useToast();
  const queryClient = useQueryClient();
  const [input, setInput] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [undoError, setUndoError] = useState<Error | null>(null);
  const [undoConfirmOpen, setUndoConfirmOpen] = useState(false);
  const [actionBusy, setActionBusy] = useState(false);
  const [actionError, setActionError] = useState<Error | null>(null);
  const [view, setView] = useState<"conversation" | "trajectory">("conversation");
  const [mode, setMode] = useState<RunMode>("yolo");
  const chatAgent = useChatAgentContext();
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
  const activeWorkspace = workspaces.data?.workspaces.find(
    (workspace) => workspace.id === workspaces.data.active_id
  );
  const timeline = useQuery({
    queryKey: ["timeline", activeSession?.id],
    queryFn: () => api.sessions.timeline(activeSession!.id),
    enabled: Boolean(activeSession)
  });
  // 轨迹视图把系统提示词作为首条记录；只在切过去时才拉，对话视图用不到
  const contextPrompt = useQuery({
    queryKey: ["session-context-prompt", activeSession?.id, "trajectory", locale, chatAgent.selection?.id, mode],
    queryFn: () => api.sessions.contextPrompt(activeSession!.id, {
      locale,
      agentId: chatAgent.selection?.id,
      mode
    }),
    enabled: Boolean(activeSession) && view === "trajectory",
    staleTime: 30_000
  });
  const onSettled = useCallback(() => {
    void Promise.all([
      activeSession?.id
        ? queryClient.invalidateQueries({ queryKey: ["timeline", activeSession.id] })
        : Promise.resolve(),
      activeSession?.id
        ? queryClient.invalidateQueries({ queryKey: ["session-debug-requests", activeSession.id] })
        : Promise.resolve(),
      activeSession?.id
        ? queryClient.invalidateQueries({ queryKey: ["session-turn-tree", activeSession.id] })
        : Promise.resolve(),
      queryClient.invalidateQueries({ queryKey: ["sessions"] }),
      queryClient.invalidateQueries({ queryKey: ["todos"] }),
      queryClient.invalidateQueries({ queryKey: ["system-usage"] })
    ]);
  }, [activeSession?.id, queryClient]);
  // 换会话回到对话视图：轨迹是针对某一次会话的分析视角，
  // 带着它进入新会话会让人以为看到的是新会话的轨迹
  useEffect(() => {
    setView("conversation");
    setTreeOpen(false);
  }, [activeSession?.id]);
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
  const onQueueMerged = useCallback(() => {
    showToast(t("A queued message was inserted into this turn", "一条排队消息已插入本轮"), "success");
  }, [showToast, t]);
  const run = useRunStream(
    workspaces.data?.active_id,
    activeSession?.id,
    onSettled,
    onWorkspaceChanged,
    onInterruptedWithoutReply,
    onQueueMerged
  );
  const turnTree = useTurnTree(activeSession?.id, {
    onBranchChanged: run.reset,
    onError: (error) => setActionError(toDisplayError(
      error,
      "Failed to switch the conversation branch",
      "切换会话分支失败"
    ))
  });
  // 运行状态在模型偏好之前计算：运行中点选的模型要暂存为待生效
  const runningStates = run.states.filter((state) => !state.completed);
  const activeRun = runningStates.find((state) => state.status !== "queued") ?? runningStates[0];
  const running = runningStates.length > 0;
  const chatModel = useChatModel(activeSession?.id, running);
  const thinking = useThinkingLevel(activeSession?.id, chatModel.thinkingLevels);
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
  const uniqueErrorNotices = useMemo(
    () => collectChatErrorNotices({ timeline: timeline.error, model: chatModel.error, action: actionError }),
    [actionError, chatModel.error, timeline.error]
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
    clearToolExpandState();
    setInput(readComposerDraft(activeSession?.id));
    setSubmittedEmptySessionId(null);
  }, [activeSession?.id]);

  // 时间线落盘后修剪已完成 live run，释放重复的 parts/tools 内存。
  // 依赖 run.states：SSE backlog 重放晚于时间线查询返回时也能补一次修剪，
  // 否则重放重建的运行会一直堆在会话底部
  const liveRunIds = run.states.map((state) => state.runId).join(",");
  useEffect(() => {
    const historyTurns = timeline.data?.turns.map((turn) => ({
      turnId: turn.turn_id,
      running: turn.status === "running"
    })) ?? [];
    if (historyTurns.length === 0) return;
    run.pruneSettled(historyTurns);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [run.pruneSettled, timeline.data?.turns, liveRunIds]);

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
    if (!input || showJump) return;
    jumpToBottom();
  }, [input, jumpToBottom, showJump]);

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
    if (turnTree.switchBranch.isPending || branchActions.pending || submitting) return;
    const value = input.trim();
    if ((!value && composerAttachments.attachments.length === 0) || !activeSession) return;
    const renameCommand = parseRenameCommand(value);
    if (renameCommand) {
      if (!renameCommand.title) {
        setActionError(new Error(t("Enter a title after /rename", "请在 /rename 后输入会话标题")));
        return;
      }
      const originalInput = input;
      setActionError(null);
      setInput("");
      clearComposerDraft(activeSession.id);
      setSubmitting(true);
      try {
        await api.sessions.rename(activeSession.id, renameCommand.title);
        await queryClient.invalidateQueries({ queryKey: ["sessions"] });
        await queryClient.invalidateQueries({ queryKey: ["session-tree"] });
      } catch (error) {
        setInput(originalInput);
        writeComposerDraft(activeSession.id, originalInput);
        setActionError(toDisplayError(error, "Failed to rename the session", "重命名会话失败"));
      } finally {
        setSubmitting(false);
      }
      return;
    }
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
      setSubmitting(true);
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
      } finally {
        setSubmitting(false);
      }
      return;
    }
    const originalInput = input;
    const currentAttachments = composerAttachments.attachments;
    if (conversationEmpty) setSubmittedEmptySessionId(activeSession.id);
    setSubmitting(true);
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
    } finally {
      setSubmitting(false);
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
    const accepted = await confirm({
      title: t("Edit and resend this turn?", "编辑并重新发送这一轮？"),
      description: t("The conversation will branch from this turn. The original turn stays in the tree.", "对话将从这一轮分出新分支。原来的一轮仍留在分支树中。"),
      confirmLabel: t("Resend", "重新发送"),
      cancelLabel: t("Cancel", "取消")
    });
    if (!accepted) return;
    setActionBusy(true);
    try {
      await resend.editAndResend(turnId, content, imageUrls);
    } finally {
      setActionBusy(false);
    }
  };

  const continueFrom = async (turnId: string) => {
    const accepted = await confirm({
      title: t("Continue from this turn?", "从这里继续？"),
      description: t("Later messages stay in the branch tree. New replies will continue from this turn.", "之后的消息仍留在分支树中。新的回复将从这一轮继续。"),
      confirmLabel: t("Continue", "继续"),
      cancelLabel: t("Cancel", "取消")
    });
    if (accepted) branchActions.continueFrom(turnId);
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
      pendingSelection={chatModel.pendingSelection}
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
      submitting={submitting}
      onChange={setInput}
      onModeChange={setMode}
      onThinkingLevelChange={thinking.setThinkingLevel}
      onAddImages={addComposerImages}
      onRemoveAttachment={composerAttachments.removeAttachment}
      onModelSelect={chatModel.selectModel}
      onSubmit={() => void submit()}
      onStop={() => {
        if (!activeRun?.runId) return;
        void run.stop(activeRun.runId).catch((error) => {
          setActionError(toDisplayError(error, "Failed to stop the run", "停止运行失败"));
        });
      }}
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
  const composerDock = (
    <div className="composer-dock">
      {uniqueErrorNotices.length > 0 && (
        <div className="composer-error-dock">
          {uniqueErrorNotices.map((notice) => (
            <RunErrorNotice key={notice.key} message={notice.message} detail={notice.detail} />
          ))}
        </div>
      )}
      {composer}
    </div>
  );

  // 有历史才提供轨迹视图：空会话切过去只有一张空表，切换本身成了噪声
  const viewSwitch = (timeline.data?.turns.length ?? 0) > 0
    ? <ChatViewSwitch view={view} onChange={setView} /> : undefined;
  const header = (
    <ChatSessionHeader
      title={conversationEmpty && activeSession ? t("New task", "新建任务") : activeSession?.title ?? t("Select a session", "选择会话")}
      workspace={activeWorkspace}
      branch={gitStatus.data?.status === "ready" ? gitStatus.data.head : undefined}
      viewSwitch={viewSwitch}
      branchNavigation={view === "conversation" && (turnTree.tree.data?.total_turns ?? 0) > 0 ? <TurnTreeNavigation open={treeOpen} count={turnTree.tree.data?.branch_points ?? 0} onToggle={() => setTreeOpen((value) => !value)} /> : undefined}
      actions={toolbar}
    />
  );

  if (view === "trajectory" && viewSwitch) {
    return (
      <div className="chat-page">
        <div className="chat-trajectory-region">
          {header}
          <TrajectoryView
            sessionId={activeSession?.id}
            timeline={timeline.data}
            contextPrompt={contextPrompt.data}
            loading={timeline.isLoading}
          />
        </div>
      </div>
    );
  }

  return (
    <div className={centerEmptySession ? "chat-page empty-session" : "chat-page"}>
      <div className="chat-page-navigation">
        {header}
        {treeOpen && turnTree.tree.data && <TurnTreePanel tree={turnTree.tree.data} busy={turnTree.switchBranch.isPending || running} onSelect={(turnId) => turnTree.switchBranch.mutate(turnId)} onClose={() => setTreeOpen(false)} />}
      </div>
      <div className="message-scroll-region">
        <div className="message-scroll" ref={scrollRef}>
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
            {!branchTransitioning && <ChatConversation
              turns={display.historyTurns}
              liveRuns={activeLiveRuns}
              running={running}
              lastTurnId={lastTurnId}
              actions={{
                sessionId: activeSession?.id,
                onRetry: resend.retry,
                canContinueFrom: !running,
                onContinueFrom: continueFrom,
                onSideConversation: openTurnSideConversation,
                canEditResend: !running,
                onEditResend: editAndResend,
                actionBusy: actionBusy || branchTransitioning,
                branchTree: turnTree.tree.data,
                branchBusy: turnTree.switchBranch.isPending || running,
                onSwitchBranch: turnTree.switchBranch.mutate
              }}
            />}
            <QueuedMessageList
              runs={queuedRuns}
              onUpdate={run.updateQueuedInput}
              onMove={run.moveQueuedRun}
              onPromote={run.promoteQueuedRun}
              onInsertAt={run.updateQueuedInsertAt}
              onRemove={run.removeQueuedRun}
              onError={(error) => setActionError(toDisplayError(error, "Failed to update the message queue", "更新消息队列失败"))}
            />
          </div>
          {centerEmptySession ? (
            <ChatEmptyState onChoose={setInput} disabled={!activeSession}>{composerDock}</ChatEmptyState>
          ) : (
            composerDock
          )}
        </div>
        <MessageOverviewRail
          scrollContainerRef={scrollRef}
          items={overviewItems}
          onNavigate={pauseFollowing}
        />
        {showJump && (
          <HoverRevealButton
            className="jump-to-bottom"
            expanded={running}
            icon={<ArrowDown size={16} />}
            label={running
              ? t("Following paused · Jump to bottom", "已暂停跟随 · 回到底部")
              : t("Jump to bottom", "回到底部")}
            onClick={jumpToBottom}
          />
        )}
      </div>
      <ChatSessionDialogs
        undoOpen={undoConfirmOpen}
        undoError={undoError}
        onCloseUndo={() => setUndoConfirmOpen(false)}
        onUndo={() => void undoToPreviousTurn()}
        onCloseError={() => setUndoError(null)}
      />
      <Toast notice={notice} onDismiss={dismissToast} />
    </div>
  );
}
