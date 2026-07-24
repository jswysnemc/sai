import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Brain, FileText, Plus, Search, Database, Trash2, Zap } from "lucide-react";
import { useState, type ReactNode } from "react";
import { api } from "../../api/client";
import type { AppConfig, MemoryEntry, MemorySearchHit, MemoryStats } from "../../api/contracts";
import { useI18n } from "../i18n/use-i18n";
import { SettingsGroup } from "./editor-layout";

type MemorySettingsSectionProps = {
  config?: AppConfig | null;
  onConfigChange?: (config: AppConfig) => void;
};

/**
 * 记忆管理：启停、统计（含 Markdown / FTS）、搜索、新增事实、删除条目。
 *
 * @param props 可选 AppConfig（用于 plugins.memory.enabled）
 * @returns 记忆设置区域
 */
export function MemorySettingsSection({ config, onConfigChange }: MemorySettingsSectionProps = {}) {
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
      if (query.trim()) await queryClient.invalidateQueries({ queryKey: ["memory-search"] });
    }
  });

  const remove = useMutation({
    mutationFn: ({ kind, id }: { kind: "fact" | "episode"; id: number }) => api.memory.remove(kind, id),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["memory-entries"] });
      await queryClient.invalidateQueries({ queryKey: ["memory-stats"] });
      if (query.trim()) await queryClient.invalidateQueries({ queryKey: ["memory-search"] });
    }
  });

  const reset = useMutation({
    mutationFn: api.memory.reset,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["memory-entries"] });
      await queryClient.invalidateQueries({ queryKey: ["memory-stats"] });
      if (query.trim()) await queryClient.invalidateQueries({ queryKey: ["memory-search"] });
    }
  });

  const facts = entries.data?.facts ?? [];
  const episodes = entries.data?.episodes ?? [];
  const storage = stats.data?.storage;

  return (
    <section className="settings-section-card">
      <header className="settings-section-head">
        <h2><Brain size={16} /> {t("Memory", "记忆管理")}</h2>
        <p>
          {t(
            "Long-term facts and events are stored as Markdown files and indexed with SQLite FTS for search. Clearing memory does not change the current conversation history.",
            "长期事实与往事以 Markdown 落盘，并用 SQLite FTS 建立全文索引；清空记忆不会改动当前会话历史。"
          )}
        </p>
      </header>

      {config && onConfigChange && (
        <SettingsGroup
          title={t("Memory feature", "记忆功能")}
          description={t(
            "When disabled, long-term memory tools and automatic session memory extraction stay off. Default is enabled.",
            "关闭后，长期记忆工具与自动会话记忆提取均不运行。默认开启。"
          )}
        >
          <label className="settings-toggle-field">
            <span>
              <strong>{t("Enable memory", "启用记忆")}</strong>
              <small>plugins.memory.enabled</small>
            </span>
            <input
              type="checkbox"
              checked={((config.plugins?.memory as { enabled?: boolean } | undefined)?.enabled) !== false}
              onChange={(event) => {
                const plugins = config.plugins ?? {};
                const prev = (plugins.memory as Record<string, unknown> | undefined) ?? {};
                onConfigChange({
                  ...config,
                  plugins: {
                    ...plugins,
                    memory: {
                      ...prev,
                      enabled: event.target.checked
                    }
                  },
                  memory: {
                    ...(config.memory as Record<string, unknown> | undefined),
                    enabled: event.target.checked
                  }
                });
              }}
            />
          </label>
        </SettingsGroup>
      )}

      <div className="memory-storage-grid">
        <StorageCard
          icon={<Database size={14} />}
          title={t("SQLite tables", "SQLite 表")}
          status={t("Canonical", "权威数据")}
          body={t(
            `${Number(stats.data?.facts ?? 0)} facts · ${Number(stats.data?.episodes ?? 0)} events`,
            `${Number(stats.data?.facts ?? 0)} 条事实 · ${Number(stats.data?.episodes ?? 0)} 条往事`
          )}
          path={shortPath(stats.data?.data_db)}
        />
        <StorageCard
          icon={<FileText size={14} />}
          title={t("Markdown source", "Markdown 源")}
          status={markdownStatus(storage, t)}
          body={t(
            `${Number(storage?.markdown_facts ?? 0)} fact files · ${Number(storage?.markdown_episodes ?? 0)} event files`,
            `${Number(storage?.markdown_facts ?? 0)} 个事实文件 · ${Number(storage?.markdown_episodes ?? 0)} 个往事文件`
          )}
          path={shortPath(stats.data?.files_dir)}
        />
        <StorageCard
          icon={<Zap size={14} />}
          title={t("FTS5 index", "FTS5 索引")}
          status={ftsStatus(storage, t)}
          body={t(
            `unicode ${Number(storage?.fts?.facts ?? 0) + Number(storage?.fts?.episodes ?? 0)} · trigram ${Number(storage?.fts?.facts_trigram ?? 0) + Number(storage?.fts?.episodes_trigram ?? 0)}`,
            `unicode ${Number(storage?.fts?.facts ?? 0) + Number(storage?.fts?.episodes ?? 0)} · trigram ${Number(storage?.fts?.facts_trigram ?? 0) + Number(storage?.fts?.episodes_trigram ?? 0)}`
          )}
          path={t("facts_fts / episodes_fts (+trigram)", "facts_fts / episodes_fts（含 trigram）")}
        />
      </div>

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
        <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("Search via FTS (facts + events)", "用 FTS 搜索事实与往事")} />
      </label>
      {query.trim() && (
        <div className="memory-search-panel">
          <div className="memory-search-meta">
            {search.isFetching
              ? t("Searching…", "搜索中…")
              : t(
                  `FTS results · ${countHits(search.data?.facts)} facts · ${countHits(search.data?.episodes)} events`,
                  `FTS 结果 · ${countHits(search.data?.facts)} 条事实 · ${countHits(search.data?.episodes)} 条往事`
                )}
          </div>
          <SearchHitList title={t("Facts", "事实")} hits={search.data?.facts ?? []} />
          <SearchHitList title={t("Events", "往事")} hits={search.data?.episodes ?? []} />
          {!search.isFetching && countHits(search.data?.facts) === 0 && countHits(search.data?.episodes) === 0 && (
            <div className="settings-muted">{t("No matches", "无匹配结果")}</div>
          )}
        </div>
      )}

      <MemoryList title={t("Facts", "事实")} items={facts} onRemove={(id) => remove.mutate({ kind: "fact", id })} />
      <MemoryList title={t("Events", "往事")} items={episodes} onRemove={(id) => remove.mutate({ kind: "episode", id })} />
      {(entries.error || stats.error || remember.error || remove.error || reset.error || search.error) && (
        <div className="pane-error">
          {((entries.error || stats.error || remember.error || remove.error || reset.error || search.error) as Error).message}
        </div>
      )}
    </section>
  );
}

function StorageCard({
  icon,
  title,
  status,
  body,
  path
}: {
  icon: ReactNode;
  title: string;
  status: string;
  body: string;
  path?: string;
}) {
  return (
    <article className="memory-storage-card">
      <header>
        <span className="memory-storage-icon">{icon}</span>
        <div>
          <strong>{title}</strong>
          <small>{status}</small>
        </div>
      </header>
      <p>{body}</p>
      {path && <code title={path}>{path}</code>}
    </article>
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
              <small>
                {item.source} · {item.status}
                {item.has_markdown ? ` · ${t("Markdown", "Markdown")}` : ""}
                {" · "}
                {new Date(item.updated_at).toLocaleString(locale)}
              </small>
              <button type="button" onClick={() => onRemove(item.id)} aria-label={t("Delete memory", "删除记忆")}>
                <Trash2 size={13} />
              </button>
            </footer>
            {item.markdown_path && (
              <code className="memory-md-path" title={item.markdown_path}>
                {shortPath(item.markdown_path)}
              </code>
            )}
          </article>
        ))}
      </div>
    </div>
  );
}

function SearchHitList({ title, hits }: { title: string; hits: MemorySearchHit[] }) {
  const { t } = useI18n();
  if (!hits.length) return null;
  return (
    <div className="memory-search-group">
      <h4>{title}</h4>
      <ul>
        {hits.map((hit) => (
          <li key={`${hit.source}-${hit.id}`}>
            <span className="memory-search-score" title={t("FTS score", "FTS 分数")}>
              {hit.score.toFixed(1)}
            </span>
            <div>
              <p>{hit.content}</p>
              <small>{hit.source} · #{hit.id}</small>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}

function countHits(hits?: MemorySearchHit[]) {
  return hits?.length ?? 0;
}

function shortPath(path?: string) {
  if (!path) return undefined;
  // 展示末两段路径，完整路径放 title
  const parts = path.split("/").filter(Boolean);
  if (parts.length <= 3) return path;
  return `…/${parts.slice(-3).join("/")}`;
}

function markdownStatus(
  storage: MemoryStats["storage"] | undefined,
  t: (en: string, zh: string) => string
) {
  const md = Number(storage?.markdown_facts ?? 0) + Number(storage?.markdown_episodes ?? 0);
  if (md > 0) return t("Synced", "已同步");
  return t("Empty / pending init", "空 / 待初始化");
}

function ftsStatus(
  storage: MemoryStats["storage"] | undefined,
  t: (en: string, zh: string) => string
) {
  if (storage?.fts?.ready) return t("Ready", "就绪");
  const total =
    Number(storage?.fts?.facts ?? 0) +
    Number(storage?.fts?.episodes ?? 0) +
    Number(storage?.fts?.facts_trigram ?? 0) +
    Number(storage?.fts?.episodes_trigram ?? 0);
  if (total > 0) return t("Building", "构建中");
  return t("Not ready", "未就绪");
}
