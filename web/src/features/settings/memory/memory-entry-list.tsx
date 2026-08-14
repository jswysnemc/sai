import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, Trash2 } from "lucide-react";
import { useState } from "react";
import { api } from "../../../api/client";
import type { MemoryScope, MemorySummary, MemoryType } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";

type MemoryEntryListProps = {
  entries: MemorySummary[];
  onRemove: (name: string) => void;
};

/** 类型徽章的双语文案。 */
const TYPE_LABELS: Record<MemoryType, { en: string; zh: string }> = {
  user: { en: "User", zh: "用户" },
  feedback: { en: "Feedback", zh: "要求" },
  project: { en: "Project", zh: "项目" },
  reference: { en: "Reference", zh: "资源" }
};

/**
 * 按作用域分组展示记忆条目。
 *
 * 项目记忆排在前面：同名时它覆盖全局那条，先看到的才是实际生效的那条。
 *
 * @param props 条目与删除回调
 * @returns 记忆列表
 */
export function MemoryEntryList({ entries, onRemove }: MemoryEntryListProps) {
  const { t } = useI18n();
  const groups: Array<{ scope: MemoryScope; title: string }> = [
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
            {items.length === 0 && <div className="settings-muted">{t("None", "暂无")}</div>}
            <div className="memory-list">
              {items.map((entry) => (
                <MemoryRow key={`${scope}-${entry.name}`} entry={entry} onRemove={onRemove} />
              ))}
            </div>
          </div>
        );
      })}
    </>
  );
}

/**
 * 单条记忆；正文按需拉取，展开才请求。
 *
 * 列表接口只带摘要：一次列出全部正文会把设置页拖慢，而多数时候用户
 * 只想确认某条存不存在。
 *
 * @param props 条目与删除回调
 * @returns 记忆行
 */
function MemoryRow({ entry, onRemove }: { entry: MemorySummary; onRemove: (name: string) => void }) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(false);
  const detail = useQuery({
    queryKey: ["memory-detail", entry.name],
    queryFn: () => api.memory.show(entry.name),
    enabled: expanded
  });

  return (
    <article className="memory-item">
      <header className="memory-item-head">
        <button
          type="button"
          className="memory-item-toggle"
          onClick={() => setExpanded((value) => !value)}
          aria-expanded={expanded}
        >
          {expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
          <code>{entry.name}</code>
        </button>
        <span className="memory-type-badge" data-type={entry.type}>
          {t(TYPE_LABELS[entry.type].en, TYPE_LABELS[entry.type].zh)}
        </span>
        <button type="button" onClick={() => onRemove(entry.name)} aria-label={t("Delete memory", "删除记忆")}>
          <Trash2 size={13} />
        </button>
      </header>
      <p>{entry.description}</p>
      {expanded && (
        <div className="memory-item-body">
          {detail.isFetching && <div className="settings-muted">{t("Loading…", "读取中…")}</div>}
          {detail.data?.found && <pre>{detail.data.content}</pre>}
          {detail.data?.found && (detail.data.links?.length ?? 0) > 0 && (
            <div className="memory-item-links">
              {t("Links", "关联")}：
              {detail.data.links?.map((link) => (
                <code key={link}>[[{link}]]</code>
              ))}
            </div>
          )}
          {detail.data && !detail.data.found && (
            <div className="settings-muted">{t("Entry file is missing", "条目文件已不存在")}</div>
          )}
        </div>
      )}
    </article>
  );
}
