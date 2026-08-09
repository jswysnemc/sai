import { useState } from "react";
import type { useConfirm } from "../../shared/ui/dialog/dialog-provider";

type ConfirmFn = ReturnType<typeof useConfirm>;

type SessionSelectionOptions = {
  confirm: ConfirmFn;
  /** 双语文本选择方法 */
  t: (en: string, zh: string) => string;
  /** 批量删除动作；由调用方绑定缓存刷新 */
  removeMany: (ids: string[]) => Promise<unknown>;
};

/**
 * 管理会话多选模式：进入退出、勾选、全选与批量删除确认。
 *
 * @param options 确认框、文本与批量删除动作
 * @returns 多选状态与操作集合
 */
export function useSessionSelection({ confirm, t, removeMany }: SessionSelectionOptions) {
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  /**
   * 切换指定会话的选中状态。
   *
   * @param id 会话 ID
   */
  const toggleSelected = (id: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
    setConfirming(false);
  };

  /**
   * 全选或取消全选给定会话集合。
   *
   * @param ids 当前工作区全部会话 ID
   * @returns 无返回值
   */
  const toggleAll = (ids: string[]) => {
    const allSelected = ids.length > 0 && ids.every((id) => selected.has(id));
    setSelected(allSelected ? new Set() : new Set(ids));
    setConfirming(false);
  };

  /**
   * 删除所选会话；先弹出危险确认，避免误触。
   *
   * @returns 无返回值
   */
  const requestBulkDelete = async () => {
    if (selected.size === 0 || busy) return;
    setConfirming(true);
    const count = selected.size;
    const accepted = await confirm({
      title: t("Delete sessions", "删除会话"),
      description: t(
        `Delete ${count} selected session(s)? This cannot be undone.`,
        `删除已选的 ${count} 个会话？此操作不可撤销。`
      ),
      confirmLabel: t("Delete", "删除"),
      cancelLabel: t("Cancel", "取消"),
      danger: true
    });
    setConfirming(false);
    if (!accepted) return;
    setBusy(true);
    try {
      await removeMany(Array.from(selected));
      setSelected(new Set());
      setSelecting(false);
    } finally {
      setBusy(false);
    }
  };

  /** 退出选择模式并清理临时状态。 */
  const exitSelection = () => {
    setSelecting(false);
    setSelected(new Set());
    setConfirming(false);
  };

  /**
   * 进入多选模式；不默认勾选任何会话。
   *
   * @returns 无返回值
   */
  const enterSelection = () => {
    setSelecting(true);
    setSelected(new Set());
    setConfirming(false);
  };

  return {
    selecting,
    selected,
    confirming,
    busy,
    toggleSelected,
    toggleAll,
    requestBulkDelete,
    enterSelection,
    exitSelection
  };
}
