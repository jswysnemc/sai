import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus } from "../../../shared/ui/icons";
import { useEffect, useMemo, useState } from "react";
import { api } from "../../../api/client";
import type { AppConfig, MemorySummary, MemoryWriteRequest, MemoryWriteResult } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { EmptyGuide, InlineSwitch } from "../kit";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";
import { MemoryComposeForm, MemoryWriteFeedback } from "./memory-compose-form";
import { MemoryEntryCard } from "./memory-entry-card";
import { MemoryEntryList } from "./memory-entry-list";
import { MemoryEvictedSearch } from "./memory-evicted-search";
import { MemoryFilterBar } from "./memory-filter-bar";
import { EMPTY_MEMORY_FILTER, filterMemories, type MemoryFilter } from "./memory-filter";
import { MemoryIndexPreview } from "./memory-index-preview";
import { MemoryMetaRow } from "./memory-meta-row";
import { useMemoryBatchDelete } from "./use-memory-batch-delete";
import { MemoryToolsRow } from "./memory-tools-row";
import "./memory-settings-section.css";

type MemorySettingsSectionProps = {
  config?: AppConfig | null;
  onConfigChange?: (config: AppConfig) => void;
};

/**
 * 记忆管理：启停、工作区选择、筛选搜索、新建、就地编辑、链接导航、
 * 注入索引预览与逐出上下文检索。
 *
 * 布局对齐文件式记忆页：左侧导航、右侧详情，筛选与统计收进顶栏。
 *
 * @param props 可选 AppConfig（用于 plugins.memory.enabled）
 * @returns 记忆设置区域
 */
export function MemorySettingsSection({ config, onConfigChange }: MemorySettingsSectionProps = {}) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [filter, setFilter] = useState<MemoryFilter>(EMPTY_MEMORY_FILTER);
  const [writeResult, setWriteResult] = useState<MemoryWriteResult | null>(null);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [composing, setComposing] = useState(false);

  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list });
  const workspaceList = workspaces.data?.workspaces ?? [];
  const [workspaceId, setWorkspaceId] = useState<string | undefined>(undefined);
  const selectedWorkspace = workspaceId ?? workspaces.data?.active_id;

  useEffect(() => {
    setWriteResult(null);
  }, [selectedWorkspace]);

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
    onSuccess: async (result, request) => {
      setWriteResult(result);
      setComposing(false);
      setSelectedKey(`${request.global ? "global" : "project"}:${request.name}`);
      await refresh();
    }
  });
  const batchDelete = useMemoryBatchDelete(refresh);
  const remove = useMutation({
    mutationFn: ({ name, scope }: MemorySummary) =>
      api.memory.remove(name, { workspace: selectedWorkspace, scope }),
    onSuccess: async (_, entry) => {
      await queryClient.invalidateQueries({ queryKey: ["memory-detail", entry.name] });
      await refresh();
    }
  });
  const reset = useMutation({ mutationFn: api.memory.reset, onSuccess: refresh });

  /**
   * 删除前确认：删除不可恢复，误触图标不该直接生效。
   *
   * @param entry 待删除条目
   * @returns 确认流程结束后返回
   */
  const removeWithConfirm = async (entry: MemorySummary) => {
    const name = entry.name;
    const confirmed = await confirm({
      title: t("Delete memory", "删除记忆"),
      description: t(
        `Delete the memory \"${name}\"? This cannot be undone.`,
        `删除记忆「${name}」？删除后无法恢复。`
      ),
      confirmLabel: t("Delete", "删除"),
      danger: true
    });
    if (confirmed) remove.mutate(entry);
  };

  /**
   * 清空前确认：范围是所有工作区的记忆，不只是当前工作区。
   *
   * @returns 确认流程结束后返回
   */
  const resetWithConfirm = async () => {
    const confirmed = await confirm({
      title: t("Clear all memories", "清空全部记忆"),
      description: t(
        "This deletes every memory across all workspaces, not just the selected one. This cannot be undone.",
        "删除所有工作区的全部记忆，不只是当前选中的工作区。删除后无法恢复。"
      ),
      confirmLabel: t("Clear all", "全部清空"),
      danger: true
    });
    if (confirmed) reset.mutate();
  };

  const allEntries = useMemo(() => entries.data?.entries ?? [], [entries.data]);
  const visibleEntries = useMemo(
    () => filterMemories(allEntries, filter),
    [allEntries, filter]
  );
  const selectedEntry = visibleEntries.find((entry) => `${entry.scope}:${entry.name}` === selectedKey) ?? null;

  useEffect(() => {
    if (composing) return;
    if (selectedEntry) return;
    const first = visibleEntries[0];
    setSelectedKey(first ? `${first.scope}:${first.name}` : null);
  }, [composing, selectedEntry, visibleEntries]);

  const error =
    entries.error || stats.error || remember.error || remove.error || reset.error || workspaces.error;

  /**
   * 链接跳转：选中目标条目；被当前筛选排除时先清筛选。
   *
   * @param name 目标标识
   * @returns 无返回值
   */
  const navigateToMemory = (name: string) => {
    const target = allEntries.find((entry) => entry.name === name);
    if (!target) return;
    if (filterMemories([target], filter).length === 0) {
      setFilter(EMPTY_MEMORY_FILTER);
    }
    setComposing(false);
    setSelectedKey(`${target.scope}:${target.name}`);
  };

  const enabled = (config?.plugins?.memory as { enabled?: boolean } | undefined)?.enabled !== false;

  /**
   * 切换记忆功能开关：同步写入插件与记忆两组配置。
   *
   * @param checked 是否启用
   * @returns 无返回值
   */
  const toggleEnabled = (checked: boolean) => {
    if (!config || !onConfigChange) return;
    const plugins = config.plugins ?? {};
    const previous = (plugins.memory as Record<string, unknown> | undefined) ?? {};
    onConfigChange({
      ...config,
      plugins: { ...plugins, memory: { ...previous, enabled: checked } },
      memory: {
        ...(config.memory as Record<string, unknown> | undefined),
        enabled: checked
      }
    });
  };

  return (
    <div className="memory-workspace">
      <header className="memory-workspace-head">
        <MemoryMetaRow
          stats={stats.data}
          workspaceCount={workspaceList.length}
          selectedWorkspace={selectedWorkspace ?? workspaceList[0]?.id}
          workspaceOptions={workspaceList.map((workspace) => ({
            value: workspace.id,
            label: workspace.name
          }))}
          notesDir={stats.data?.notes_dir}
          onWorkspaceChange={(value) => setWorkspaceId(value || undefined)}
        />
        <div className="memory-workspace-actions">
          {config && onConfigChange && (
            <InlineSwitch
              label={t("Enable memory", "启用记忆")}
              checked={enabled}
              onChange={toggleEnabled}
            />
          )}
          <Button
            variant="secondary"
            onClick={() => {
              setComposing(true);
              setSelectedKey(null);
            }}
          >
            <Plus size={14} />
            {t("New memory", "新建记忆")}
          </Button>
          <Button
            variant="ghost-danger"
            onClick={() => void resetWithConfirm()}
            disabled={reset.isPending}
            title={t("Clears memories in every workspace, not just the selected one.", "清空的是所有工作区的记忆，不只是当前选中的工作区。")}
          >
            {reset.isPending ? t("Clearing…", "清空中…") : t("Clear all", "全部清空")}
          </Button>
        </div>
      </header>

      <div className="memory-workspace-body">
        <aside className="memory-workspace-nav">
          <MemoryFilterBar
            entries={allEntries}
            type={filter.type}
            scope={filter.scope}
            query={filter.query}
            onTypeChange={(type) => setFilter((current) => ({ ...current, type }))}
            onScopeChange={(scope) => setFilter((current) => ({ ...current, scope }))}
            onQueryChange={(query) => setFilter((current) => ({ ...current, query }))}
          />
          {filter.type !== "all" && visibleEntries.length > 0 && (
            <Button
              variant="ghost-danger"
              disabled={batchDelete.pending}
              onClick={() => void batchDelete.removeDisplayed(visibleEntries, selectedWorkspace)}
            >
              {t(`Delete ${visibleEntries.length} displayed entries`, `删除展示的 ${visibleEntries.length} 条记忆`)}
            </Button>
          )}
          {batchDelete.error && <div className="settings-inline-error">{batchDelete.error}</div>}
          <MemoryEntryList
            entries={visibleEntries}
            loading={entries.isLoading}
            selectedKey={selectedKey}
            onSelect={(entry) => {
              setComposing(false);
              setSelectedKey(`${entry.scope}:${entry.name}`);
            }}
          />
        </aside>

        <section className="memory-workspace-detail">
          {composing ? (
            <MemoryComposeForm
              pending={remember.isPending}
              workspace={selectedWorkspace}
              onCollapse={() => setComposing(false)}
              onSubmit={async (request) => {
                try {
                  return await remember.mutateAsync(request);
                } catch {
                  return null;
                }
              }}
            />
          ) : selectedEntry ? (
            <MemoryEntryCard
              key={`${selectedEntry.scope}-${selectedEntry.name}`}
              entry={selectedEntry}
              workspace={selectedWorkspace}
              forceOpen
              onRemove={removeWithConfirm}
              onNavigate={navigateToMemory}
            />
          ) : (
            <EmptyGuide
              title={t("No memory selected", "尚未选择记忆")}
              description={t("Select an entry on the left, or create a new memory.", "在左侧选择一条记忆，或新建一条。")}
            />
          )}
          <MemoryWriteFeedback result={writeResult} />
          <MemoryToolsRow>
            <MemoryIndexPreview workspace={selectedWorkspace} />
            <MemoryEvictedSearch />
          </MemoryToolsRow>
        </section>
      </div>

      {error && <div className="settings-inline-error">{(error as Error).message}</div>}
    </div>
  );
}
