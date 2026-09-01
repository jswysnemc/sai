import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Brain, Plus } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "../../../api/client";
import type { AppConfig, MemoryWriteRequest, MemoryWriteResult } from "../../../api/contracts";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";
import { MemoryComposeForm, MemoryWriteFeedback } from "./memory-compose-form";
import { MemoryEntryList } from "./memory-entry-list";
import { MemoryEvictedSearch } from "./memory-evicted-search";
import { MemoryFilterBar } from "./memory-filter-bar";
import { EMPTY_MEMORY_FILTER, filterMemories, type MemoryFilter } from "./memory-filter";
import { MemoryIndexPreview } from "./memory-index-preview";
import { MemoryMetaRow } from "./memory-meta-row";
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
 * 空间分配以条目列表为中心：列表紧随元信息行出现在首屏，新建表单默认
 * 折叠成按钮，索引预览与逐出检索收进底部工具区，启停开关下沉到页脚。
 *
 * 项目记忆按工作区分目录存放，所有请求都带上选中的工作区标识——
 * 缺省时服务端会退回它自己的 cwd，把别的工作区的记忆显示出来。
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
  const [navigateTarget, setNavigateTarget] = useState<{ name: string; signal: number } | null>(null);
  // 新建表单默认收起：录入是低频动作，不能让表单常驻挤占条目列表
  const [composing, setComposing] = useState(false);

  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list });
  const workspaceList = workspaces.data?.workspaces ?? [];
  const [workspaceId, setWorkspaceId] = useState<string | undefined>(undefined);
  const selectedWorkspace = workspaceId ?? workspaces.data?.active_id;

  // 切换工作区时清掉上一工作区的写入提示：语境已变，残留只会误导
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

  /** 删除前确认：删除不可恢复，误触图标不该直接生效。 */
  const removeWithConfirm = async (name: string) => {
    const confirmed = await confirm({
      title: t("Delete memory", "删除记忆"),
      description: t(
        `Delete the memory \"${name}\"? This cannot be undone.`,
        `删除记忆「${name}」？删除后无法恢复。`
      ),
      confirmLabel: t("Delete", "删除"),
      danger: true
    });
    if (confirmed) remove.mutate(name);
  };

  /** 清空前确认：范围是所有工作区的记忆，不只是当前工作区。 */
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

  const error =
    entries.error || stats.error || remember.error || remove.error || reset.error || workspaces.error;

  /** 链接跳转：展开目标条目；被当前筛选排除时先清筛选，否则列表不渲染它。 */
  const navigateToMemory = (name: string) => {
    const target = allEntries.find((entry) => entry.name === name);
    if (!target) return;
    // 目标被类型、作用域或关键字筛选掉时必须清掉，否则跳转毫无反应
    if (filterMemories([target], filter).length === 0) {
      setFilter(EMPTY_MEMORY_FILTER);
    }
    setNavigateTarget({ name, signal: Date.now() });
  };

  const enabled = (config?.plugins?.memory as { enabled?: boolean } | undefined)?.enabled !== false;

  /** 切换记忆功能开关：同步写入插件与记忆两组配置，保持两侧语义一致。 */
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
    <section className="settings-section-card">
      <header className="settings-section-head">
        <h2>
          <Brain size={16} /> {t("Memory", "记忆管理")}
        </h2>
      </header>

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

      {composing ? (
        <MemoryComposeForm
          pending={remember.isPending}
          workspace={selectedWorkspace}
          onCollapse={() => setComposing(false)}
          onSubmit={async (request) => {
            try {
              return await remember.mutateAsync(request);
            } catch {
              // 错误已由 error 汇总展示；返回空让表单保留输入
              return null;
            }
          }}
        />
      ) : (
        <button
          type="button"
          className="settings-secondary memory-compose-toggle"
          onClick={() => setComposing(true)}
        >
          <Plus size={14} /> {t("New memory", "新建记忆")}
        </button>
      )}
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
        loading={entries.isLoading}
        workspace={selectedWorkspace}
        onRemove={removeWithConfirm}
        onNavigate={navigateToMemory}
        navigateTarget={navigateTarget}
      />

      <MemoryToolsRow>
        <MemoryIndexPreview workspace={selectedWorkspace} />
        <MemoryEvictedSearch />
      </MemoryToolsRow>

      <div className="memory-footer-row">
        {config && onConfigChange && (
          <label className="settings-toggle-field memory-enabled-toggle">
            <span>
              <strong>{t("Enable memory", "启用记忆")}</strong>
              <small>{t("Disabled: tools unregistered, index not injected", "关闭后不注册记忆工具，也不注入索引")}</small>
            </span>
            <input
              type="checkbox"
              checked={enabled}
              onChange={(event) => toggleEnabled(event.target.checked)}
            />
          </label>
        )}
        <button
          type="button"
          className="settings-danger"
          onClick={resetWithConfirm}
          disabled={reset.isPending}
          title={t("Clears memories in every workspace, not just the selected one.", "清空的是所有工作区的记忆，不只是当前选中的工作区。")}
        >
          {reset.isPending ? t("Clearing…", "清空中…") : t("Clear all memories", "清空全部记忆")}
        </button>
      </div>

      {error && <div className="settings-inline-error">{(error as Error).message}</div>}
    </section>
  );
}
