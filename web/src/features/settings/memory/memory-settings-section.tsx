import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Brain, FolderTree, Globe, Layers } from "lucide-react";
import { useMemo, useState } from "react";
import { api } from "../../../api/client";
import type { AppConfig, MemoryWriteRequest, MemoryWriteResult } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsGroup } from "../editor-layout";
import { MemoryComposeForm, MemoryWriteFeedback } from "./memory-compose-form";
import { MemoryEntryList } from "./memory-entry-list";
import { MemoryEvictedSearch } from "./memory-evicted-search";
import { MemoryFilterBar } from "./memory-filter-bar";
import { EMPTY_MEMORY_FILTER, filterMemories, type MemoryFilter } from "./memory-filter";
import { MemoryIndexPreview } from "./memory-index-preview";
import "./memory-settings-section.css";

type MemorySettingsSectionProps = {
  config?: AppConfig | null;
  onConfigChange?: (config: AppConfig) => void;
};

/**
 * 记忆管理：启停、工作区选择、筛选搜索、新建、就地编辑、链接导航、
 * 注入索引预览与逐出上下文检索。
 *
 * 项目记忆按工作区分目录存放，所有请求都带上选中的工作区标识——
 * 缺省时服务端会退回它自己的 cwd，把别的工作区的记忆显示出来。
 *
 * @param props 可选 AppConfig（用于 plugins.memory.enabled）
 * @returns 记忆设置区域
 */
export function MemorySettingsSection({ config, onConfigChange }: MemorySettingsSectionProps = {}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [filter, setFilter] = useState<MemoryFilter>(EMPTY_MEMORY_FILTER);
  const [writeResult, setWriteResult] = useState<MemoryWriteResult | null>(null);
  const [navigateTarget, setNavigateTarget] = useState<{ name: string; signal: number } | null>(null);

  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list });
  const workspaceList = workspaces.data?.workspaces ?? [];
  const [workspaceId, setWorkspaceId] = useState<string | undefined>(undefined);
  const selectedWorkspace = workspaceId ?? workspaces.data?.active_id;

  const stats = useQuery({
    queryKey: ["memory-stats", selectedWorkspace],
    queryFn: () => api.memory.stats({ workspace: selectedWorkspace })
  });
  const entries = useQuery({
    queryKey: ["memory-entries", selectedWorkspace],
    queryFn: () => api.memory.list(200, { workspace: selectedWorkspace })
  });

  /** 写入或删除后刷新列表与统计。 */
  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: ["memory-entries"] });
    await queryClient.invalidateQueries({ queryKey: ["memory-stats"] });
    await queryClient.invalidateQueries({ queryKey: ["memory-index"] });
  };

  const remember = useMutation({
    mutationFn: (request: MemoryWriteRequest) => api.memory.remember(request),
    onSuccess: async (result) => {
      setWriteResult(result);
      await refresh();
    }
  });
  const remove = useMutation({
    mutationFn: (name: string) =>
      api.memory.remove(name, { workspace: selectedWorkspace }),
    onSuccess: async (_, name) => {
      await queryClient.invalidateQueries({ queryKey: ["memory-detail", name] });
      await refresh();
    }
  });
  const reset = useMutation({ mutationFn: api.memory.reset, onSuccess: refresh });

  const allEntries = useMemo(() => entries.data?.entries ?? [], [entries.data]);
  const visibleEntries = useMemo(
    () => filterMemories(allEntries, filter),
    [allEntries, filter]
  );

  const error =
    entries.error || stats.error || remember.error || remove.error || reset.error || workspaces.error;

  /** 链接跳转：展开目标条目；筛掉了就先清筛选。 */
  const navigateToMemory = (name: string) => {
    const known = allEntries.some((entry) => entry.name === name);
    setFilter((current) => (known ? current : EMPTY_MEMORY_FILTER));
    setNavigateTarget({ name, signal: Date.now() });
  };

  return (
    <section className="settings-section-card">
      <header className="settings-section-head">
        <h2>
          <Brain size={16} /> {t("Memory", "记忆管理")}
        </h2>
        <p>
          {t(
            "Each memory is one markdown file holding one fact. The index is injected every turn; the assistant reads an entry's body on demand. Files can be edited by hand and kept in version control.",
            "每条记忆是一个 markdown 文件，只放一个事实。索引每轮注入，正文由助手按需读取。文件可以手改，也可以纳入版本控制。"
          )}
        </p>
      </header>

      {config && onConfigChange && (
        <SettingsGroup
          title={t("Memory feature", "记忆功能")}
          description={t(
            "When disabled, the memory tools are not registered and the index is not injected. Default is enabled.",
            "关闭后不注册记忆工具，也不注入索引。默认开启。"
          )}
        >
          <label className="settings-toggle-field">
            <span>
              <strong>{t("Enable memory", "启用记忆")}</strong>
              <small>plugins.memory.enabled</small>
            </span>
            <input
              type="checkbox"
              checked={(config.plugins?.memory as { enabled?: boolean } | undefined)?.enabled !== false}
              onChange={(event) => {
                const plugins = config.plugins ?? {};
                const previous = (plugins.memory as Record<string, unknown> | undefined) ?? {};
                onConfigChange({
                  ...config,
                  plugins: { ...plugins, memory: { ...previous, enabled: event.target.checked } },
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

      {workspaceList.length > 1 && (
        <label className="memory-workspace-field">
          <span>{t("Workspace", "工作区")}</span>
          <select
            value={selectedWorkspace ?? ""}
            onChange={(event) => setWorkspaceId(event.target.value || undefined)}
          >
            {workspaceList.map((workspace) => (
              <option key={workspace.id} value={workspace.id}>
                {workspace.name}
              </option>
            ))}
          </select>
          <small>
            {t(
              "Project memories are stored per workspace",
              "项目记忆按工作区存放"
            )}
          </small>
        </label>
      )}

      <div className="memory-storage-grid">
        <StatCard
          icon={<FolderTree size={14} />}
          title={t("Project", "项目记忆")}
          value={stats.data?.project_memories ?? 0}
          hint={t("Visible only in this workspace", "仅在当前工作区可见")}
        />
        <StatCard
          icon={<Globe size={14} />}
          title={t("Global", "全局记忆")}
          value={stats.data?.global_memories ?? 0}
          hint={t("Applies everywhere", "在所有工作区生效")}
        />
        <StatCard
          icon={<Layers size={14} />}
          title={t("Evicted turns", "逐出轮次")}
          value={stats.data?.evicted_turns ?? 0}
          hint={t("Originals kept after compaction", "压缩后留档的原文")}
        />
      </div>

      {stats.data?.notes_dir && (
        <code className="memory-notes-path" title={stats.data.notes_dir}>
          {stats.data.notes_dir}
        </code>
      )}

      <MemoryComposeForm
        pending={remember.isPending}
        workspace={selectedWorkspace}
        onSubmit={(request) => remember.mutate(request)}
      />
      <MemoryWriteFeedback result={writeResult} />

      <MemoryFilterBar
        entries={allEntries}
        type={filter.type}
        scope={filter.scope}
        query={filter.query}
        onTypeChange={(type) => setFilter((current) => ({ ...current, type }))}
        onScopeChange={(scope) => setFilter((current) => ({ ...current, scope }))}
        onQueryChange={(query) => setFilter((current) => ({ ...current, query }))}
      />

      <MemoryEntryList
        entries={visibleEntries}
        workspace={selectedWorkspace}
        onRemove={(name) => remove.mutate(name)}
        onNavigate={navigateToMemory}
        navigateTarget={navigateTarget}
      />

      <MemoryIndexPreview workspace={selectedWorkspace} />

      <MemoryEvictedSearch />

      <div className="memory-danger-row">
        <button
          type="button"
          className="settings-secondary"
          onClick={() => reset.mutate()}
          disabled={reset.isPending}
        >
          {reset.isPending ? t("Clearing…", "清空中…") : t("Clear all memories", "清空全部记忆")}
        </button>
      </div>

      {error && <div className="settings-inline-error">{(error as Error).message}</div>}
    </section>
  );
}

/**
 * 一张统计卡片。
 *
 * @param props 图标、标题、数值与说明
 * @returns 统计卡片
 */
function StatCard({
  icon,
  title,
  value,
  hint
}: {
  icon: React.ReactNode;
  title: string;
  value: number;
  hint: string;
}) {
  return (
    <article className="memory-storage-card">
      <header>
        <span className="memory-storage-icon">{icon}</span>
        <div>
          <strong>{title}</strong>
          <small>{hint}</small>
        </div>
      </header>
      <p className="memory-stat-value">{value}</p>
    </article>
  );
}
