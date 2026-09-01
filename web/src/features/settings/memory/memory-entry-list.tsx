import type { MemorySummary } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { MemoryEntryCard } from "./memory-entry-card";

type MemoryEntryListProps = {
  entries: MemorySummary[];
  /** 列表仍在加载时为真：空列表不能先显示「暂无」再突然变出条目 */
  loading: boolean;
  workspace?: string;
  onRemove: (name: string) => void;
  /** 点击 [[链接]] 时跳转到目标条目 */
  onNavigate: (name: string) => void;
  /** 链接跳转的目标条目与触发序号 */
  navigateTarget: { name: string; signal: number } | null;
};

/**
 * 按作用域分组展示记忆条目。
 *
 * 项目记忆排在前面：同名时它覆盖全局那条，先看到的才是实际生效的那条。
 * 筛选与搜索由外层完成，这里只负责分组与渲染。
 *
 * @param props 条目、加载状态、工作区标识、操作回调与跳转目标
 * @returns 记忆列表
 */
export function MemoryEntryList({
  entries,
  loading,
  workspace,
  onRemove,
  onNavigate,
  navigateTarget
}: MemoryEntryListProps) {
  const { t } = useI18n();
  const groups: Array<{ scope: "project" | "global"; title: string }> = [
    { scope: "project", title: t("Project memories", "项目记忆") },
    { scope: "global", title: t("Global memories", "全局记忆") }
  ];

  return (
    <>
      {groups.map(({ scope, title }) => {
        const items = entries.filter((entry) => entry.scope === scope);
        return (
          <div key={scope} className="memory-list-block">
            <h3>
              {title} ({items.length})
            </h3>
            {loading && items.length === 0 && (
              <div className="settings-muted">{t("Loading…", "读取中…")}</div>
            )}
            {!loading && items.length === 0 && (
              <div className="settings-muted">{t("None", "暂无")}</div>
            )}
            <div className="memory-list">
              {items.map((entry) => (
                <MemoryEntryCard
                  key={`${scope}-${entry.name}`}
                  entry={entry}
                  workspace={workspace}
                  onRemove={onRemove}
                  onNavigate={onNavigate}
                  expandSignal={navigateTarget?.name === entry.name ? navigateTarget.signal : 0}
                />
              ))}
            </div>
          </div>
        );
      })}
    </>
  );
}
