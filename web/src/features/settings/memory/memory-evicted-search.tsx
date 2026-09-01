import { useQuery } from "@tanstack/react-query";
import { History, Search } from "lucide-react";
import { useDeferredValue, useState } from "react";
import { api } from "../../../api/client";
import { useI18n } from "../../i18n/use-i18n";

/**
 * 检索被压缩清出上下文的对话轮次。
 *
 * 这是压缩摘要的补救途径：摘要有损，原文并未删除。摘要末尾那句回读指引
 * 指向的就是这份数据，这里给人一个同样的入口。
 *
 * 默认只显示一个入口按钮，展开后才出现搜索框与结果——
 * 诊断场景低频，不该常驻占据列表空间。
 *
 * @returns 逐出上下文检索面板
 */
export function MemoryEvictedSearch() {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  // 键入中途不发请求：每个按键都触发一次后端扫描纯属浪费
  const deferredQuery = useDeferredValue(query);
  const search = useQuery({
    queryKey: ["memory-evicted", deferredQuery],
    queryFn: () => api.memory.searchEvicted(deferredQuery, 20),
    enabled: open && deferredQuery.trim().length > 0
  });
  const results = search.data?.results ?? [];

  if (!open) {
    return (
      <div className="memory-tool-entry">
        <button
          type="button"
          className="memory-tool-toggle"
          onClick={() => setOpen(true)}
        >
          <History size={13} />
          {t("Evicted context", "逐出上下文")}
          <small>{t("Search turns removed by compaction", "检索被压缩清出的轮次")}</small>
        </button>
      </div>
    );
  }

  return (
    <div className="memory-tool-panel">
      <button
        type="button"
        className="memory-tool-toggle"
        onClick={() => setOpen(false)}
        aria-expanded="true"
      >
        <History size={13} />
        {t("Evicted context", "逐出上下文")}
      </button>
      <label className="memory-search">
        <Search size={14} />
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("Search context removed by compaction", "检索被压缩清出的上下文")}
        />
      </label>
      {deferredQuery.trim() && (
        <div className="memory-search-panel">
          <div className="memory-search-meta">
            {search.isFetching
              ? t("Searching…", "检索中…")
              : t(`${results.length} matching turns`, `命中 ${results.length} 条轮次`)}
          </div>
          <ul className="memory-evicted-list">
            {results.map((hit) => (
              <li key={hit.id}>
                <span className="memory-search-score" title={t("Match score", "匹配得分")}>
                  {hit.score.toFixed(1)}
                </span>
                <div>
                  <p>{hit.snippet}</p>
                  <small>
                    {hit.role} · {hit.timestamp}
                  </small>
                </div>
              </li>
            ))}
          </ul>
          {!search.isFetching && results.length === 0 && (
            <div className="settings-muted">{t("No matches", "无匹配结果")}</div>
          )}
        </div>
      )}
    </div>
  );
}
