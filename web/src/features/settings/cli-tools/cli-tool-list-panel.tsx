import { useMemo, useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { ObjectListPanel } from "../object-list-panel";
import {
  cliToolCategoryLabel,
  cliToolLabel,
  getCliToolCatalogEntry
} from "./cli-tool-catalog";
import { useI18n } from "../../i18n/use-i18n";

type CliToolStatusFilter = "all" | "enabled" | "disabled";

type CliToolListItem = {
  id: string;
  config: Record<string, unknown>;
};

type CliToolListPanelProps = {
  tools: CliToolListItem[];
  selectedId: string;
  onSelect: (id: string) => void;
};

/**
 * 渲染 CLI 助手工具列表，并提供启用状态筛选。
 *
 * @param props 工具配置、当前选择和选择回调
 * @returns 可搜索、可筛选的工具列表
 */
export function CliToolListPanel({ tools, selectedId, onSelect }: CliToolListPanelProps) {
  const { locale, t } = useI18n();
  const [status, setStatus] = useState<CliToolStatusFilter>("all");

  // 1. 状态筛选只改变左侧列表，不修改工具配置
  const visibleTools = useMemo(
    () => tools.filter(({ config }) => {
      const enabled = config.enabled !== false;
      return status === "all" || (status === "enabled" ? enabled : !enabled);
    }),
    [status, tools]
  );

  return (
    <ObjectListPanel
      title={t("CLI assistant tools", "CLI 助手工具")}
      items={visibleTools.map(({ id, config }) => {
        const entry = getCliToolCatalogEntry(id);
        const Icon = entry.icon;
        const enabled = config.enabled !== false;
        const state = enabled ? t("Enabled", "已启用") : t("Disabled", "已停用");
        return {
          id,
          name: cliToolLabel(entry, locale),
          meta: cliToolCategoryLabel(entry.category, locale) + " · " + state,
          icon: <Icon size={14} />,
          marked: enabled
        };
      })}
      selectedId={selectedId}
      searchPlaceholder={t("Search CLI tools", "搜索 CLI 助手工具")}
      topSlot={(
        <div className="cli-tool-filter" aria-label={t("Filter tools by status", "按状态筛选工具")}>
          <FilterButton
            active={status === "all"}
            label={t("All", "全部")}
            onClick={() => setStatus("all")}
          />
          <FilterButton
            active={status === "enabled"}
            label={t("Enabled", "启用")}
            onClick={() => setStatus("enabled")}
          />
          <FilterButton
            active={status === "disabled"}
            label={t("Disabled", "停用")}
            onClick={() => setStatus("disabled")}
          />
        </div>
      )}
      onSelect={onSelect}
    />
  );
}

/**
 * 渲染工具状态筛选按钮。
 *
 * @param props 激活状态、按钮文案和点击回调
 * @returns 统一样式筛选按钮
 */
function FilterButton({
  active,
  label,
  onClick
}: {
  active: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button
      variant="secondary"
      className={active ? "active" : ""}
      aria-pressed={active}
      onClick={onClick}
    >
      {label}
    </Button>
  );
}
