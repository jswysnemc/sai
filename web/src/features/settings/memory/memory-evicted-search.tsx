import { useQuery } from "@tanstack/react-query";
import { Search } from "lucide-react";
import { useState } from "react";
import { api } from "../../../api/client";
import { useI18n } from "../../i18n/use-i18n";

/**
 * 检索被压缩清出上下文的对话轮次。
 *
 * 这是压缩摘要的补救途径：摘要有损，原文并未删除。摘要末尾那句回读指引
 * 指向的就是这份数据，这里给人一个同样的入口。
 *
 * @returns 逐出上下文检索面板
 */
export function MemoryEvictedSearch() {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const search = useQuery({
    queryKey: ["memory-evicted", query],
    queryFn: () => api.memory.searchEvicted(query, 20),
    enabled: query.trim().length > 0
  });
  const results = search.data?.results ?? [];

  return (
    <div className="memory-evicted">
      <label className="memory-search">
        <Search size={14} />
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("Search context removed by compaction", "检索被压缩清出的上下文")}
        />
      </label>
      {query.trim() && (
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
