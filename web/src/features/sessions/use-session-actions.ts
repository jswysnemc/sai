import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { WorkspaceSessions } from "../../api/contracts";
import type { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { switchWithTerminalConfirm } from "../workspaces/workspace-switcher";
import { invalidateWorkspaceContext } from "../workspaces/invalidate-workspace-context";
import { initializeNewSessionPreferences } from "./new-session-preferences";
import { describeRunningBackgroundWork, loadRunningBackgroundWork } from "./session-close-guard";
import { commitLocalSessionSelection, enqueueSessionNavigation } from "./session-navigation";

type ConfirmFn = ReturnType<typeof useConfirm>;

type SessionActionsOptions = {
  confirm: ConfirmFn;
  /** 双语文本选择方法 */
  t: (en: string, zh: string) => string;
  /** 当前会话树数据，用于关闭工作区时挑选回退目标 */
  tree: () => WorkspaceSessions[] | undefined;
  /** 打开会话后的导航回调（如关闭移动端抽屉） */
  onNavigate?: () => void;
  /** 在当前浏览器标签页记录会话选择，不修改其他标签页共享的服务端指针 */
  onSessionSelected?: (workspaceId: string, sessionId: string) => void;
};

/**
 * 聚合会话侧栏的全部数据操作：创建、打开、重命名、删除与工作区切换。
 *
 * 侧栏壳组件只负责布局与视图状态，把「做什么」集中到这里，
 * 行与节点组件拿到的都是已经绑好确认框与缓存刷新的动作。
 *
 * @param options 确认框、文本、树数据与导航回调
 * @returns 会话与工作区操作集合
 */
export function useSessionActions({ confirm, t, tree, onNavigate, onSessionSelected }: SessionActionsOptions) {
  const queryClient = useQueryClient();
  const [navigationError, setNavigationError] = useState<Error | null>(null);

  /**
   * 【会话】【缓存刷新】刷新会话列表和全部消息缓存。
   *
   * @returns 全部相关缓存刷新完成后返回
   */
  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    await queryClient.invalidateQueries({ queryKey: ["session-tree"] });
    await queryClient.invalidateQueries({ queryKey: ["messages"] });
    await queryClient.invalidateQueries({ queryKey: ["timeline"] });
    // 后台任务与子智能体都按会话隔离，切会话后必须重取；
    // 这两个 queryKey 不含会话维度，不主动失效就会继续显示上一个会话的列表
    await queryClient.invalidateQueries({ queryKey: ["background-tasks"] });
    await queryClient.invalidateQueries({ queryKey: ["subagents"] });
  };

  /**
   * 切换工作区和会话；工作区变化只刷新相关查询，会话变化只更新当前标签页。
   *
   * @param workspaceId 目标工作区 ID
   * @param sessionId 目标会话 ID
   * @param workspaceActive 目标工作区是否已经激活
   * @returns 切换流程完成后返回
   */
  const openSession = async (
    workspaceId: string,
    sessionId: string,
    workspaceActive: boolean,
    _sessionActive: boolean
  ) => {
    setNavigationError(null);
    await enqueueSessionNavigation(queryClient, async (navigation) => {
      try {
        const active = navigation.workspaceActive(workspaceId, workspaceActive);
        if (!active) {
          const switched = await switchWithTerminalConfirm(workspaceId, confirm, t);
          if (!switched) return;
        }
        navigation.selectWorkspace(workspaceId);
        if (!navigation.isCurrent()) return;
        // 1. 【会话导航】【标签页选择】会话选择保存在当前标签页，避免覆盖其他并行会话的服务端指针
        onSessionSelected?.(workspaceId, sessionId);
        commitLocalSessionSelection(queryClient, workspaceId, sessionId);
        void api.sessionSidebar.update({ clear_unread: sessionId }).then(() => {
          void queryClient.invalidateQueries({ queryKey: ["session-sidebar"] });
        });
        onNavigate?.();
        if (!active) {
          await invalidateWorkspaceContext(queryClient);
        }
      } catch (cause) {
        if (navigation.isCurrent()) setNavigationError(toDisplayError(cause, "Failed to open session", "打开会话失败"));
      }
    });
  };

  /** 切换到指定工作区；工作区视图不强制打开某个会话。 */
  const openWorkspace = async (workspaceId: string, workspaceActive: boolean) => {
    if (workspaceActive) return;
    setNavigationError(null);
    try {
      const switched = await switchWithTerminalConfirm(workspaceId, confirm, t);
      if (switched) {
        await invalidateWorkspaceContext(queryClient);
      }
    } catch (cause) {
      setNavigationError(toDisplayError(cause, "Failed to open workspace", "打开工作区失败"));
    }
  };

  /**
   * 【会话】【新会话默认值】创建会话并在列表刷新前写入专属模型与思考偏好。
   *
   * @param workspaceId 可选目标工作区 ID
   * @returns 新建会话
   */
  const createSession = async (workspaceId?: string) => {
    const response = await queryClient.ensureQueryData({
      queryKey: ["config"],
      queryFn: api.config.load
    });
    const engine = response.config.agent?.engine ?? "native";
    // 1. 【会话】【新会话默认值】外部内核先读取当前能力，失败时按内核默认值创建
    const status = engine === "native"
      ? undefined
      : await queryClient.fetchQuery({
          queryKey: ["engine-status"],
          queryFn: api.config.engineStatus
        }).catch(() => undefined);
    // 2. 【会话】【新会话默认值】服务端创建成功后立即建立会话专属偏好
    const session = await api.sessions.create(undefined, workspaceId);
    initializeNewSessionPreferences(session.id, response.config, status);
    return session;
  };

  const create = useMutation({
    mutationFn: createSession,
    onSuccess: async (session, workspaceId) => {
      // 1. 先刷新会话树，使新会话立即出现在目标工作区
      await refresh();
      const activeWorkspaceId = tree()?.find((workspace) => workspace.active)?.workspace_id;
      const targetWorkspaceId = workspaceId ?? activeWorkspaceId;
      if (!targetWorkspaceId) return;
      // 2. 非活动工作区先切换工作区，再激活刚创建的会话
      await openSession(targetWorkspaceId, session.id, workspaceId === undefined, session.active);
    }
  });
  const remove = useMutation({ mutationFn: api.sessions.remove, onSuccess: refresh });
  const rename = useMutation({
    mutationFn: ({ id, title }: { id: string; title: string }) => api.sessions.rename(id, title),
    onSuccess: refresh
  });
  const removeMany = useMutation({
    mutationFn: api.sessions.removeMany,
    onSuccess: refresh
  });
  const removeWorkspace = useMutation({
    mutationFn: api.workspaces.remove,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["workspaces"] });
      await queryClient.invalidateQueries({ queryKey: ["session-tree"] });
    }
  });

  /**
   * 确认后删除单个会话。
   *
   * @param sessionId 会话 ID
   * @param title 会话标题，用于确认文案
   * @returns 无返回值
   */
  const removeWithConfirm = async (sessionId: string, title: string) => {
    const running = await loadRunningBackgroundWork(sessionId);
    const accepted = await confirm({
      title: running.length ? t("Close session with background work?", "仍要关闭会话？") : t("Delete session", "删除会话"),
      description: describeRunningBackgroundWork(
        running,
        t(`Delete “${title}”? This cannot be undone.`, `删除“${title}”？此操作不可撤销。`),
        t
      ),
      confirmLabel: running.length ? t("Close anyway", "仍要关闭") : t("Delete", "删除"),
      cancelLabel: t("Cancel", "取消"),
      danger: true
    });
    if (accepted) remove.mutate(sessionId);
  };

  /** 确认后关闭非活动工作区；活动工作区先切换到回退工作区再关闭。 */
  const closeWorkspace = async (workspaceId: string, workspaceName: string, workspaceActive: boolean) => {
    setNavigationError(null);
    try {
      const sessions = tree()?.find((workspace) => workspace.workspace_id === workspaceId)?.sessions ?? [];
      const running = (await Promise.all(sessions.map((session) => loadRunningBackgroundWork(session.id)))).flat();
      const accepted = await confirm({
        title: running.length ? t("Close workspace with background work?", "仍要关闭工作区？") : t("Close workspace", "关闭工作区"),
        description: describeRunningBackgroundWork(
          running,
          t(
            `Close “${workspaceName}” from the list? Workspace files will not be deleted.`,
            `从列表中关闭“${workspaceName}”？工作区文件不会被删除。`
          ),
          t
        ),
        confirmLabel: running.length ? t("Close anyway", "仍要关闭") : t("Close", "关闭")
      });
      if (!accepted) return;
      if (workspaceActive) {
        const fallback = tree()?.find((workspace) => workspace.workspace_id !== workspaceId);
        if (!fallback) return;
        const switched = await switchWithTerminalConfirm(fallback.workspace_id, confirm, t);
        if (!switched) return;
        await api.workspaces.remove(workspaceId);
        await invalidateWorkspaceContext(queryClient);
        return;
      }
      removeWorkspace.mutate(workspaceId);
    } catch (cause) {
      setNavigationError(toDisplayError(cause, "Failed to close workspace", "关闭工作区失败"));
    }
  };

  /**
   * 登记服务端目录并切换到对应工作区。
   *
   * @param path 服务端目录路径
   */
  const openDirectory = async (path: string) => {
    const workspace = await api.workspaces.add(path);
    const switched = await switchWithTerminalConfirm(workspace.id, confirm, t);
    if (switched) {
      await invalidateWorkspaceContext(queryClient);
    }
  };

  const error =
    navigationError
    ?? create.error
    ?? remove.error
    ?? removeMany.error
    ?? rename.error
    ?? removeWorkspace.error;

  return {
    create,
    remove,
    rename,
    removeMany,
    openSession,
    openWorkspace,
    closeWorkspace,
    openDirectory,
    removeWithConfirm,
    error
  };
}
