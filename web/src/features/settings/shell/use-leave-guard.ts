import { useEffect, useRef } from "react";
import { useBlocker } from "react-router-dom";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";
import { leavesSettingsDraft, useSettingsDrafts } from "./settings-draft-context";

/**
 * 【设置】【离开确认】统一保护全局草稿与即将卸载的独立文档。
 * @param dirty 全局草稿是否未保存
 * @param onDiscard 放弃全局草稿的操作
 * @returns 无返回值；导航阻止由路由器统一执行
 */
export function useLeaveGuard(dirty: boolean, onDiscard: () => void) {
  const drafts = useSettingsDrafts();
  const confirm = useConfirm();
  const { t } = useI18n();
  const pending = useRef(false);
  const blocker = useBlocker(({ nextLocation }) => {
    const leavesSettings = !/^\/settings(?:\/|$)/.test(nextLocation.pathname);
    return (dirty && leavesSettings) || drafts.some((draft) => leavesSettingsDraft(draft, nextLocation));
  });
  useEffect(() => {
    if (blocker.state !== "blocked") { pending.current = false; return; }
    if (pending.current) return;
    pending.current = true;
    void confirm({ title: t("Leave with unsaved changes", "有未保存的修改"), description: t("Unsaved changes in the document you are leaving will be discarded.", "离开当前文档会丢弃尚未保存的修改。"), confirmLabel: t("Discard and leave", "放弃并离开"), danger: true }).then((accepted) => {
      if (!accepted) { blocker.reset(); return; }
      // 1. 仅放弃即将卸载的草稿，分区间切换仍保留全局配置
      if (!/^\/settings(?:\/|$)/.test(blocker.location.pathname)) onDiscard();
      for (const draft of drafts) if (leavesSettingsDraft(draft, blocker.location)) draft.discard();
      blocker.proceed();
    });
  }, [blocker, confirm, drafts, onDiscard, t]);

  const anyDirty = dirty || drafts.some((draft) => draft.dirty);
  useEffect(() => {
    if (!anyDirty) return;
    /** 阻止直接刷新或关闭导致草稿丢失；参数为卸载事件，返回无值。 */
    const beforeUnload = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ""; };
    window.addEventListener("beforeunload", beforeUnload);
    return () => window.removeEventListener("beforeunload", beforeUnload);
  }, [anyDirty]);
}
