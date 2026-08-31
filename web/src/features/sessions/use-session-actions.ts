import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { WorkspaceSessions } from "../../api/contracts";
import type { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { switchWithTerminalConfirm } from "../workspaces/workspace-switcher";
import { initializeNewSessionPreferences } from "./new-session-preferences";

type ConfirmFn = ReturnType<typeof useConfirm>;

type SessionActionsOptions = {
  confirm: ConfirmFn;
  /** 双语文本选择方法 */
  t: (en: string, zh: string) => string;
  /** 当前会话树数据，用于关闭工作区时挑选回退目标 */
  tree: () => WorkspaceSessions[] | undefined;
  /** 打开会话后的导航回调（如关闭移动端抽屉） */
  onNavigate?: () => void;
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
export function useSessionActions({ confirm, t, tree, onNavigate }: SessionActionsOptions) {
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
   * 切换工作区和会话，跨工作区时完成切换后重新载入工作台。
   *
   * @param workspaceId 目标工作区 ID
   * @param sessionId 目标会话 ID
   * @param workspaceActive 目标工作区是否已经激活
   * @param sessionActive 目标会话是否已经激活
   * @returns 切换流程完成后返回
   */
  const openSession = async (
    workspaceId: string,
    sessionId: string,
    workspaceActive: boolean,
    sessionActive: boolean
  ) => {
    setNavigationError(null);
    try {
      if (sessionActive) {
        onNavigate?.();
        return;
      }
      if (!workspaceActive) {
        const switched = await switchWithTerminalConfirm(workspaceId, confirm, t);
        if (!switched) return;
      }
      await api.sessions.switch(sessionId);
      await queryClient.invalidateQueries({ queryKey: ["workspaces"] });
      await refresh();
      onNavigate?.();
    } catch (cause) {
      setNavigationError(toDisplayError(cause, "Failed to open session", "打开会话失败"));
    }
  };

  /** 切换到指定工作区；工作区视图不强制打开某个会话。 */
  const openWorkspace = async (workspaceId: string, workspaceActive: boolean) => {
    if (workspaceActive) return;
    setNavigationError(null);
    try {
      const switched = await switchWithTerminalConfirm(workspaceId, confirm, t);
      if (switched) window.location.reload();
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
    const accepted = await confirm({
      title: t("Delete session", "删除会话"),
      description: t(`Delete “${title}”? This cannot be undone.`, `删除“${title}”？此操作不可撤销。`),
      confirmLabel: t("Delete", "删除"),
      cancelLabel: t("Cancel", "取消"),
      danger: true
    });
    if (accepted) remove.mutate(sessionId);
  };

  /** 确认后关闭非活动工作区；活动工作区先切换到回退工作区再关闭。 */
  const closeWorkspace = async (workspaceId: string, workspaceName: string, workspaceActive: boolean) => {
    setNavigationError(null);
    try {
      const accepted = await confirm({
        title: t("Close workspace", "关闭工作区"),
        description: t(
          `Close “${workspaceName}” from the list? Workspace files will not be deleted.`,
          `从列表中关闭“${workspaceName}”？工作区文件不会被删除。`
        ),
        confirmLabel: t("Close", "关闭")
      });
      if (!accepted) return;
      if (workspaceActive) {
        const fallback = tree()?.find((workspace) => workspace.workspace_id !== workspaceId);
        if (!fallback) return;
        const switched = await switchWithTerminalConfirm(fallback.workspace_id, confirm, t);
        if (!switched) return;
        await api.workspaces.remove(workspaceId);
        window.location.reload();
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
    if (switched) window.location.reload();
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
