import { CheckCheck, Search, Timer, X } from "lucide-react";
import { useMemo, useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { AgentToolOption } from "./agents-types";
import {
  countToolModes,
  expandWildcard,
  resolveToolMode,
  updateToolModes,
  type ToolMode,
  type ToolModeSelection
} from "./agent-tool-mode-state";
import { AgentToolModeGroup, type ToolModeLabels } from "./agent-tool-mode-group";
import "./agent-permissions.css";

type AgentToolPermissionsProps = {
  /** 全部可用工具 */
  tools: AgentToolOption[];
  /** 当前启用的工具名称 */
  enabled: string[];
  /** 当前需要 load 才暴露的工具名称 */
  deferred: string[];
  /** 工具权限变化回调 */
  onChange: (enabledTools: string[], deferredTools: string[]) => void;
};

/** 状态筛选档位，all 表示不筛选 */
type ToolStatusFilter = "all" | ToolMode;

/**
 * 渲染可搜索、可分组批量设置的 Agent 工具三段权限面板。
 *
 * 三段语义：on 会话开始即暴露，load 由模型按需加载，off 完全不注册。
 *
 * @param props 工具列表、两类已启用名称和变化回调
 * @returns 工具权限面板
 */
export function AgentToolPermissions({ tools, enabled, deferred, onChange }: AgentToolPermissionsProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<ToolStatusFilter>("all");
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const selection: ToolModeSelection = { enabled, deferred };
  const allNames = tools.map((tool) => tool.name);
  const nonBaseNames = tools.filter((tool) => tool.group !== "base").map((tool) => tool.name);

  const modeLabels: ToolModeLabels = [
    { value: "on", label: t("On", "启用"), title: t("Exposed from the start of the session", "会话开始即暴露给模型") },
    { value: "load", label: t("Load", "按需"), title: t("Exposed after the model calls load", "模型调用 load 后才暴露") },
    { value: "off", label: t("Off", "关闭"), title: t("Never registered for this Agent", "完全不注册给该 Agent") }
  ];

  const statusFilters: Array<{ value: ToolStatusFilter; label: string }> = [
    { value: "all", label: t("All", "全部") },
    { value: "on", label: t("On", "启用") },
    { value: "load", label: t("Load", "按需") },
    { value: "off", label: t("Off", "关闭") }
  ];

  /**
   * 应用一次状态变更，并在逐项调整时把通配符展开为具体名称。
   *
   * @param names 本次需要更新的工具名称
   * @param mode 目标状态
   */
  const applyChange = (names: string[], mode: ToolMode) => {
    const expanded = expandWildcard(selection, nonBaseNames);
    const next = updateToolModes(expanded, names, mode, allNames);
    onChange(next.enabled, next.deferred);
  };

  const isBase = useMemo(() => {
    const baseNames = new Set(tools.filter((tool) => tool.group === "base").map((tool) => tool.name));
    return (name: string) => baseNames.has(name);
  }, [tools]);

  /** 按分组整理工具，并根据搜索词与状态筛选可见项。 */
  const groups = useMemo(() => {
    const grouped = new Map<string, AgentToolOption[]>();
    for (const tool of tools) {
      const items = grouped.get(tool.group) ?? [];
      items.push(tool);
      grouped.set(tool.group, items);
    }
    return [...grouped.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([group, items]) => {
        const label = items[0]?.group_label || group;
        return {
          group,
          label,
          allItems: items,
          visibleItems: items.filter((tool) => {
            const matchesQuery = normalizedQuery.length === 0
              || tool.name.toLocaleLowerCase().includes(normalizedQuery)
              || group.toLocaleLowerCase().includes(normalizedQuery)
              || (tool.group_label ?? "").toLocaleLowerCase().includes(normalizedQuery)
              || (tool.description ?? "").toLocaleLowerCase().includes(normalizedQuery);
            const matchesStatus = statusFilter === "all"
              || resolveToolMode(selection, tool.name, tool.group === "base") === statusFilter;
            return matchesQuery && matchesStatus;
          })
        };
      })
      .filter(({ visibleItems }) => visibleItems.length > 0);
  }, [deferred, enabled, normalizedQuery, statusFilter, tools]);

  if (tools.length === 0) {
    return <p className="agent-permissions-empty">{t("No tools available.", "暂无可用工具。")}</p>;
  }

  const counts = countToolModes(selection, allNames, isBase);

  return (
    <div className="agent-permissions-panel agent-tool-permissions">
      <div className="agent-permissions-toolbar">
        <label className="agent-permissions-search">
          <Search size={14} aria-hidden="true" />
          <input
            type="search"
            value={query}
            placeholder={t("Search tools, groups, or descriptions", "搜索工具、分组或说明")}
            aria-label={t("Search tools, groups, or descriptions", "搜索工具、分组或说明")}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
        <div className="agent-tool-status-filter" role="radiogroup" aria-label={t("Filter tool status", "筛选工具状态")}>
          {statusFilters.map((item) => (
            <button
              key={item.value}
              type="button"
              role="radio"
              aria-checked={statusFilter === item.value}
              data-active={statusFilter === item.value ? "true" : "false"}
              onClick={() => setStatusFilter(item.value)}
            >
              {item.label}
            </button>
          ))}
        </div>
        <span className="agent-permissions-summary">
          {t(
            `On ${counts.on} · Load ${counts.load} · Off ${counts.off}`,
            `启用 ${counts.on} · 按需 ${counts.load} · 关闭 ${counts.off}`
          )}
        </span>
        <div className="agent-permissions-actions">
          <Button onClick={() => applyChange(allNames, "on")}>
            <CheckCheck size={14} aria-hidden="true" />
            {t("All on", "全部启用")}
          </Button>
          <Button onClick={() => applyChange(nonBaseNames, "load")}>
            <Timer size={14} aria-hidden="true" />
            {t("Non-base to load", "非基础按需")}
          </Button>
          <Button onClick={() => onChange([], [])} disabled={enabled.length === 0 && deferred.length === 0}>
            <X size={14} aria-hidden="true" />
            {t("Reset", "重置")}
          </Button>
        </div>
      </div>

      {groups.length === 0 ? (
        <p className="agent-permissions-empty">{t("No matching tools.", "没有匹配的工具。")}</p>
      ) : (
        <div className="agent-tool-permission-groups">
          {groups.map(({ group, label, allItems, visibleItems }) => (
            <AgentToolModeGroup
              key={group}
              group={group}
              label={label}
              allItems={allItems}
              visibleItems={visibleItems}
              selection={selection}
              modeLabels={modeLabels}
              groupAriaLabel={t(`Set permissions for the ${label} group`, `设置${label}分组的权限`)}
              itemAriaLabel={(name) => t(`Set permissions for ${name}`, `设置 ${name} 的权限`)}
              onChange={applyChange}
            />
          ))}
        </div>
      )}
    </div>
  );
}
