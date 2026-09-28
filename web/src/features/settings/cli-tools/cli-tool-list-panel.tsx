import { useMemo, useState } from "react";
import { ChoicePills, LocalTabs, SkTextInput } from "../kit";
import {
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
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<CliToolStatusFilter>("all");

  // 1. 状态筛选只改变导航列表，不修改工具配置
  const visibleTools = useMemo(
    () => tools.filter(({ id, config }) => {
      const enabled = config.enabled !== false;
      const matches = `${id} ${cliToolLabel(getCliToolCatalogEntry(id), locale)}`.toLowerCase().includes(query.trim().toLowerCase());
      return matches && (status === "all" || (status === "enabled" ? enabled : !enabled));
    }),
    [locale, query, status, tools]
  );

  return <div className="grid min-w-0 gap-2">
    <div className="flex flex-wrap items-center gap-2">
      <div className="w-full sm:max-w-64"><SkTextInput type="search" value={query} onChange={setQuery} aria-label={t("Search CLI tools", "搜索 CLI 助手工具")} placeholder={t("Search CLI tools", "搜索 CLI 助手工具")} /></div>
      <ChoicePills value={status} options={[{ value: "all", label: t("All", "全部") }, { value: "enabled", label: t("Enabled", "启用") }, { value: "disabled", label: t("Disabled", "停用") }]} onChange={setStatus} ariaLabel={t("Filter tools by status", "按状态筛选工具")} />
    </div>
    <LocalTabs value={selectedId} items={visibleTools.map(({ id }) => { const entry = getCliToolCatalogEntry(id); const Icon = entry.icon; return { id, label: cliToolLabel(entry, locale), icon: <Icon size={14} /> }; })} onChange={onSelect} ariaLabel={t("CLI assistant tools", "CLI 助手工具")} />
    {!visibleTools.length && <p className="m-0 text-xs text-muted">{t("No matching tools", "没有匹配的工具")}</p>}
  </div>;
}
