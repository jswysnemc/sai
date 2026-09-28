import { useState } from "react";
import { api } from "../../../api/client";
import type { MemorySummary } from "../../../api/contracts";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";

/**
 * 【记忆】【批量删除】确认当前展示范围后逐项删除，保留各条目作用域。
 * @param refresh 删除完成后的缓存刷新回调
 * @returns 删除动作、执行状态与错误信息
 */
export function useMemoryBatchDelete(refresh: () => Promise<void>) {
  const confirm = useConfirm();
  const { t } = useI18n();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  /** 确认并删除快照中的条目；参数为条目与工作区，返回完成的 Promise。 */
  const removeDisplayed = async (entries: readonly MemorySummary[], workspace?: string) => {
    if (!entries.length || pending) return;
    const targets = [...entries];
    const names = targets.map((entry) => `${entry.scope === "global" ? t("Global", "全局") : t("Project", "项目")}: ${entry.name}`).join("\n");
    if (!await confirm({ title: t(`Delete ${targets.length} displayed memories?`, `删除当前展示的 ${targets.length} 条记忆？`), description: `${t("Only these entries will be deleted. This cannot be undone.", "仅删除以下条目，操作无法恢复。")}\n${names}`, confirmLabel: t("Delete displayed", "删除展示条目"), danger: true })) return;
    setPending(true);
    setError("");
    const failed: string[] = [];
    try {
      // 1. 顺序更新索引，避免同一目录的并发读改写相互覆盖
      for (const entry of targets) {
        try { await api.memory.remove(entry.name, { workspace, scope: entry.scope }); }
        catch { failed.push(entry.name); }
      }
      // 2. 即使部分请求失败，也刷新成功删除的结果
      await refresh();
      if (failed.length) setError(t(`Failed to delete: ${failed.join(", ")}`, `删除失败：${failed.join("、")}`));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally { setPending(false); }
  };
  return { removeDisplayed, pending, error };
}
