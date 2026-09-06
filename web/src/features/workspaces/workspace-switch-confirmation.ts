import { api } from "../../api/client";
import type { Translate } from "../i18n/i18n-context";
import { getUnsavedEditorPaths } from "../workspace/unsaved-editor-changes";

type ConfirmWorkspaceSwitch = (options: {
  title: string;
  description: string;
  confirmLabel?: string;
  danger?: boolean;
}) => Promise<boolean>;

/**
 * 切换工作区前确认未保存修改，遇到终端占用时经确认后关闭终端重试。
 * @param id 目标工作区 ID
 * @param confirm 全局确认对话框方法
 * @param t 双语文本选择方法
 * @returns 是否完成切换；取消时保留当前工作区与编辑内容
 */
export async function switchWithTerminalConfirm(
  id: string,
  confirm: ConfirmWorkspaceSwitch,
  t: Translate
): Promise<boolean> {
  // 1. 【工作区】【切换确认】先确认未保存编辑，取消时不请求后端切换
  const unsavedPaths = getUnsavedEditorPaths();
  if (unsavedPaths.length > 0) {
    const confirmed = await confirm({
      title: t("Discard unsaved edits and switch workspace", "丢弃未保存修改并切换工作区"),
      description: `${t("Save these files before switching, or discard their edits to continue:", "以下文件尚未保存。请取消切换并先保存，或丢弃修改后继续：")}\n${unsavedPaths.join("\n")}`,
      confirmLabel: t("Discard and switch", "丢弃并切换"),
      danger: true
    });
    if (!confirmed) return false;
  }

  try {
    // 2. 【工作区】【切换确认】尝试普通切换，后端负责检查终端占用
    await api.workspaces.switch(id);
    return true;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (!message.includes("terminal")) throw error;
    // 3. 【工作区】【切换确认】仅在用户确认后关闭终端并重试
    const confirmed = await confirm({
      title: t("Close terminals and switch workspace", "关闭终端并切换工作区"),
      description: t("Terminal sessions are running. Close all terminals and switch workspace?", "当前有终端会话在运行，关闭全部终端并切换？"),
      confirmLabel: t("Close and switch", "关闭并切换"),
      danger: true
    });
    if (!confirmed) return false;
    await api.workspaces.switch(id, true);
    return true;
  }
}
