import { useCallback, useEffect, type MouseEvent } from "react";
import { useNavigate } from "react-router-dom";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";

/**
 * 【Web 设置】【离开确认】有未保存修改时拦截离开设置页。
 *
 * 关闭或刷新标签页由浏览器的 beforeunload 提示拦截；
 * 应用内的离开链接通过返回的点击处理函数弹出统一确认对话框。
 *
 * @param dirty 全局草稿是否有未保存修改
 * @param onDiscard 用户确认离开时放弃草稿
 * @returns 离开链接的点击处理函数，参数为点击事件与目标地址
 */
export function useLeaveGuard(dirty: boolean, onDiscard: () => void) {
  const confirm = useConfirm();
  const navigate = useNavigate();
  const { t } = useI18n();

  useEffect(() => {
    if (!dirty) return;
    /**
     * 阻止带未保存修改的页面被直接关闭或刷新。
     *
     * @param event 卸载前事件
     * @returns 无返回值
     */
    const handleBeforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => window.removeEventListener("beforeunload", handleBeforeUnload);
  }, [dirty]);

  return useCallback(async (event: MouseEvent<HTMLAnchorElement>, to: string) => {
    if (!dirty) return;
    // 1. 有修改时先拦下默认跳转，等待用户确认
    event.preventDefault();
    const confirmed = await confirm({
      title: t("Leave with unsaved changes", "有未保存的修改"),
      description: t(
        "Leaving settings discards the changes that have not been saved.",
        "离开设置页会丢弃尚未保存的修改。"
      ),
      confirmLabel: t("Discard and leave", "放弃并离开"),
      danger: true
    });
    if (!confirmed) return;
    // 2. 确认后放弃草稿再跳转
    onDiscard();
    navigate(to);
  }, [confirm, dirty, navigate, onDiscard, t]);
}
