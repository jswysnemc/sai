import type { MemorySummary } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";

type MemoryEntryListProps = {
  entries: MemorySummary[];
  /** 列表仍在加载时为真：空列表不能先显示「暂无」再突然变出条目 */
  loading: boolean;
  selectedKey?: string | null;
  onSelect: (entry: MemorySummary) => void;
};

/** 类型徽章的双语文案。 */
const TYPE_LABELS: Record<MemorySummary["type"], { en: string; zh: string }> = {
  user: { en: "User", zh: "用户" },
  feedback: { en: "Feedback", zh: "要求" },
  project: { en: "Project", zh: "项目" },
  reference: { en: "Reference", zh: "资源" }
};

/**
 * 按作用域分组展示记忆条目导航。
 *
 * 项目记忆排在前面：同名时它覆盖全局那条，先看到的才是实际生效的那条。
 * 筛选与搜索由外层完成，这里只负责分组与选中。
 *
 * @param props 条目、加载状态、当前选中与选中回调
 * @returns 记忆导航列表
 */
export function MemoryEntryList({
  entries,
  loading,
  selectedKey,
  onSelect
}: MemoryEntryListProps) {
  const { t } = useI18n();
  const groups: Array<{ scope: "project" | "global"; title: string }> = [
    { scope: "project", title: t("Project", "项目") },
    { scope: "global", title: t("Global", "全局") }
  ];

  return (
    <div className="memory-nav">
      {groups.map(({ scope, title }) => {
        const items = entries.filter((entry) => entry.scope === scope);
        return (
          <section key={scope} className="memory-nav-group">
            <h3>
              {title}
              <small>{items.length}</small>
            </h3>
            {loading && items.length === 0 && (
              <div className="settings-muted">{t("Loading…", "读取中…")}</div>
            )}
            {!loading && items.length === 0 && (
              <div className="settings-muted">{t("None", "暂无")}</div>
            )}
            <div className="memory-nav-list">
              {items.map((entry) => (
                <button
                  key={`${scope}-${entry.name}`}
                  type="button"
                  className={`memory-nav-row${selectedKey === `${entry.scope}:${entry.name}` ? " is-current" : ""}`}
                  onClick={() => onSelect(entry)}
                >
                  <code>{entry.name}</code>
                  <span>{entry.description}</span>
                  <em>{t(TYPE_LABELS[entry.type].en, TYPE_LABELS[entry.type].zh)}</em>
                </button>
              ))}
            </div>
          </section>
        );
      })}
    </div>
  );
}
