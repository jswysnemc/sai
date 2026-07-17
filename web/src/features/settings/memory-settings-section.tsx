import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Brain, Plus, Search, Trash2 } from "lucide-react";
import { useState } from "react";
import { api } from "../../api/client";
import type { MemoryEntry } from "../../api/contracts";
import { useI18n } from "../i18n/use-i18n";

/**
 * 记忆管理：查看统计、搜索、新增事实、删除条目。
 */
export function MemorySettingsSection() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [query, setQuery] = useState("");
  const [draft, setDraft] = useState("");
  const stats = useQuery({ queryKey: ["memory-stats"], queryFn: api.memory.stats });
  const entries = useQuery({ queryKey: ["memory-entries"], queryFn: () => api.memory.list(100) });
  const search = useQuery({
    queryKey: ["memory-search", query],
    queryFn: () => api.memory.search(query, 20, false),
    enabled: query.trim().length > 0
  });

  const remember = useMutation({
    mutationFn: () => api.memory.remember(draft.trim()),
    onSuccess: async () => {
      setDraft("");
      await queryClient.invalidateQueries({ queryKey: ["memory-entries"] });
      await queryClient.invalidateQueries({ queryKey: ["memory-stats"] });
    }
  });

  const remove = useMutation({
    mutationFn: ({ kind, id }: { kind: "fact" | "episode"; id: number }) => api.memory.remove(kind, id),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["memory-entries"] });
      await queryClient.invalidateQueries({ queryKey: ["memory-stats"] });
    }
  });

  const reset = useMutation({
    mutationFn: api.memory.reset,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["memory-entries"] });
      await queryClient.invalidateQueries({ queryKey: ["memory-stats"] });
    }
  });

  const facts = entries.data?.facts ?? [];
  const episodes = entries.data?.episodes ?? [];

  return (
    <section className="settings-section-card">
      <header className="settings-section-head">
        <h2><Brain size={16} /> {t("Memory", "记忆管理")}</h2>
        <p>{t("View, search, and manage long-term facts and events without changing the current conversation history.", "查看、搜索和管理长期事实与往事；不会改动当前会话历史。")}</p>
      </header>

      <div className="memory-stats">
        <div>{t("Facts", "事实")}：{String(stats.data?.facts ?? "—")}</div>
        <div>{t("Events", "往事")}：{String(stats.data?.episodes ?? "—")}</div>
        <div>{t("Evicted context", "裁剪上下文")}：{String(stats.data?.evicted_turns ?? "—")}</div>
        <button type="button" className="settings-secondary" onClick={() => reset.mutate()} disabled={reset.isPending}>
          {reset.isPending ? t("Clearing…", "清空中…") : t("Clear memory", "清空记忆")}
        </button>
      </div>

      <div className="memory-compose">
        <textarea value={draft} onChange={(e) => setDraft(e.target.value)} placeholder={t("Write a new fact…", "写入一条新事实…")} rows={3} />
        <button type="button" onClick={() => remember.mutate()} disabled={!draft.trim() || remember.isPending}>
          <Plus size={14} /> {remember.isPending ? t("Saving", "保存中") : t("Remember", "记住")}
        </button>
      </div>

      <label className="memory-search">
        <Search size={14} />
        <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("Search memory", "搜索记忆")} />
      </label>
      {query.trim() && (
        <pre className="memory-search-result">{JSON.stringify(search.data ?? {}, null, 2)}</pre>
      )}

      <MemoryList title={t("Facts", "事实")} items={facts} onRemove={(id) => remove.mutate({ kind: "fact", id })} />
      <MemoryList title={t("Events", "往事")} items={episodes} onRemove={(id) => remove.mutate({ kind: "episode", id })} />
      {(entries.error || stats.error || remember.error || remove.error || reset.error) && (
        <div className="pane-error">
          {((entries.error || stats.error || remember.error || remove.error || reset.error) as Error).message}
        </div>
      )}
    </section>
  );
}

function MemoryList({
  title,
  items,
  onRemove
}: {
  title: string;
  items: MemoryEntry[];
  onRemove: (id: number) => void;
}) {
  const { locale, t } = useI18n();
  return (
    <div className="memory-list-block">
      <h3>{title} ({items.length})</h3>
      {items.length === 0 && <div className="settings-muted">{t("None", "暂无")}</div>}
      <div className="memory-list">
        {items.map((item) => (
          <article key={`${item.kind}-${item.id}`} className="memory-item">
            <p>{item.content}</p>
            <footer>
              <small>{item.source} · {item.status} · {new Date(item.updated_at).toLocaleString(locale)}</small>
              <button type="button" onClick={() => onRemove(item.id)} aria-label={t("Delete memory", "删除记忆")}>
                <Trash2 size={13} />
              </button>
            </footer>
          </article>
        ))}
      </div>
    </div>
  );
}
