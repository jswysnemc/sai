import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Brain, FolderTree, Globe, Layers } from "lucide-react";
import { api } from "../../../api/client";
import type { AppConfig, MemoryWriteRequest } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsGroup } from "../editor-layout";
import { MemoryComposeForm } from "./memory-compose-form";
import { MemoryEntryList } from "./memory-entry-list";
import { MemoryEvictedSearch } from "./memory-evicted-search";
import "./memory-settings-section.css";

type MemorySettingsSectionProps = {
  config?: AppConfig | null;
  onConfigChange?: (config: AppConfig) => void;
};

/**
 * 记忆管理：启停、统计、新建、按作用域列出与删除、逐出上下文检索。
 *
 * @param props 可选 AppConfig（用于 plugins.memory.enabled）
 * @returns 记忆设置区域
 */
export function MemorySettingsSection({ config, onConfigChange }: MemorySettingsSectionProps = {}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const stats = useQuery({ queryKey: ["memory-stats"], queryFn: api.memory.stats });
  const entries = useQuery({ queryKey: ["memory-entries"], queryFn: () => api.memory.list(200) });

  /** 写入或删除后刷新列表与统计。 */
  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: ["memory-entries"] });
    await queryClient.invalidateQueries({ queryKey: ["memory-stats"] });
  };

  const remember = useMutation({
    mutationFn: (request: MemoryWriteRequest) => api.memory.remember(request),
    onSuccess: refresh
  });
  const remove = useMutation({
    mutationFn: (name: string) => api.memory.remove(name),
    onSuccess: async (_, name) => {
      await queryClient.invalidateQueries({ queryKey: ["memory-detail", name] });
      await refresh();
    }
  });
  const reset = useMutation({ mutationFn: api.memory.reset, onSuccess: refresh });

  const error = entries.error || stats.error || remember.error || remove.error || reset.error;

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

      <MemoryComposeForm pending={remember.isPending} onSubmit={(request) => remember.mutate(request)} />

      <MemoryEntryList entries={entries.data?.entries ?? []} onRemove={(name) => remove.mutate(name)} />

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
