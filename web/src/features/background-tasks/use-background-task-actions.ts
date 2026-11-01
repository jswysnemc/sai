import { useRef } from "react";
import type { BackgroundTask } from "../../api/contracts";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { useToast } from "../../shared/ui/notify/notify";
import { useI18n } from "../i18n/use-i18n";
import type { useBackgroundTasks } from "./use-background-tasks";

type TaskOperations = Pick<ReturnType<typeof useBackgroundTasks>, "stop" | "cleanup" | "refresh">;

/**
 * 【后台任务】【管理操作】统一确认、重复操作保护和成功或失败提示。
 * @param manager 后台任务停止、清理及刷新方法
 * @returns 操作方法和统一提示组件需要的状态
 */
export function useBackgroundTaskActions(manager: TaskOperations) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const toast = useToast();
  const busy = useRef(false);

  /**
   * 【后台任务】【操作反馈】执行单项操作，捕获错误并阻止重复提交。
   * @param action 返回提示文案的异步操作，返回空值表示用户取消
   * @returns 操作及提示处理完成后结束
   */
  const execute = async (action: () => Promise<string | null>) => {
    if (busy.current) return;
    busy.current = true;
    toast.dismissToast();
    try {
      const message = await action();
      if (message) toast.showToast(message);
    } catch (error) {
      toast.showToast(error instanceof Error ? error.message : t("Task operation failed", "任务操作失败"), "error");
    } finally {
      busy.current = false;
    }
  };

  /**
   * 【后台任务】【停止确认】确认任务名称及命令后停止进程。
   * @param task 需要停止的后台任务
   * @param force 是否跳过宽限期并强制停止
   * @returns 确认和停止操作完成后结束
   */
  const stop = (task: BackgroundTask, force = false) => execute(async () => {
    const accepted = await confirm({
      title: force ? t("Force stop background task?", "强制停止后台任务？") : t("Stop background task?", "停止后台任务？"),
      description: force
        ? t(`Force stop "${task.label}" immediately? The command will have no grace period to finish cleanup.\n${task.command}`, `立即强制停止“${task.label}”？命令将没有宽限期完成清理。\n${task.command}`)
        : t(`Stop "${task.label}"? The running command will be terminated.\n${task.command}`, `停止“${task.label}”后将终止正在执行的命令。\n${task.command}`),
      confirmLabel: force ? t("Force stop", "强制停止") : t("Stop task", "停止任务"), danger: true
    });
    if (!accepted) return null;
    const result = await manager.stop(force ? { id: task.id, force: true } : task.id);
    return result.was_running ? (force ? t("Task force stopped", "任务已强制停止") : t("Task stopped", "任务已停止")) : t("Task has already ended", "任务已经结束");
  });

  /**
   * 【后台任务】【清理确认】仅清理已结束记录，保留运行中的任务和日志文件。
   * @returns 确认和清理操作完成后结束
   */
  const cleanup = () => execute(async () => {
    const accepted = await confirm({
      title: t("Clean finished tasks?", "清理已结束任务？"),
      description: t("Remove all finished task records. Running tasks and log files will be kept.", "将移除所有已结束任务记录，保留运行中的任务和日志文件。"),
      confirmLabel: t("Clean records", "清理记录"), danger: true
    });
    if (!accepted) return null;
    const result = await manager.cleanup();
    return t(`Cleaned ${result.removed.length} task records`, `已清理 ${result.removed.length} 条任务记录`);
  });

  /**
   * 【后台任务】【刷新反馈】手动刷新任务列表及输出，并显示结果。
   * @returns 刷新完成后结束
   */
  const refresh = () => execute(async () => {
    await manager.refresh();
    return t("Tasks refreshed", "任务已刷新");
  });

  return { stop, cleanup, refresh, notice: toast.notice, dismissToast: toast.dismissToast };
}
