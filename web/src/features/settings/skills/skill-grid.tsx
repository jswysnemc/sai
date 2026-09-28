import { Plus, RefreshCw } from "../../../shared/ui/icons";
import { useMemo, useState } from "react";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { Button } from "../../../shared/ui/button/button";
import { ChoicePills, DataTable, EmptyGuide, SettingsPanel, SkSelect, SkTextInput, StatusBadge } from "../kit";
import { useI18n } from "../../i18n/use-i18n";
import { SkillCard } from "./skill-card";
import { filterManagedSkills, skillScopeLabel, type SkillStatusFilter } from "./skill-list-filter";
import type { SkillLibraryFilters } from "./skill-view-state";
import "./skill-grid.css";

type SkillGridProps = {
  skills: ManagedSkill[];
  filters: SkillLibraryFilters;
  scanning: boolean;
  error: string | null;
  onFiltersChange: (filters: SkillLibraryFilters) => void;
  onOpen: (id: string) => void;
  onAdd: () => void;
  onScan: () => void;
};

/**
 * 渲染带搜索和筛选功能的响应式 Skill 网格。
 *
 * @param props Skill 数据、筛选条件、请求状态与操作回调
 * @returns 技能库网格页
 */
export function SkillGrid({ skills, filters, scanning, error, onFiltersChange, onOpen, onAdd, onScan }: SkillGridProps) {
  const { t } = useI18n();
  const [layout, setLayout] = useState<"grid" | "list">("grid");
  const scopes = useMemo(() => [...new Set(skills.map((skill) => skill.scope))].sort(), [skills]);
  const visibleSkills = useMemo(
    () => filterManagedSkills(skills, filters.query, filters.status, filters.scope),
    [filters.query, filters.scope, filters.status, skills]
  );
  const statusOptions = [
    { value: "all", label: t("All statuses", "全部状态") },
    { value: "enabled", label: t("Enabled", "已启用") },
    { value: "disabled", label: t("Disabled", "已禁用") }
  ] satisfies Array<{ value: SkillStatusFilter; label: string }>;
  const scopeOptions = [
    { value: "all", label: t("All sources", "全部来源") },
    ...scopes.map((value) => ({ value, label: skillScopeLabel(value, t) }))
  ];
  const resultCount = visibleSkills.length === skills.length
    ? String(skills.length)
    : `${visibleSkills.length}/${skills.length}`;

  return <SettingsPanel title={t("Library", "技能库")} description={t(`${resultCount} results`, `${resultCount} 项`)} actions={<>
    <ChoicePills value={layout} onChange={setLayout} ariaLabel={t("Library view", "技能库视图")} options={[{ value: "grid", label: t("Grid", "网格") }, { value: "list", label: t("List", "列表") }]} />
    <Button variant="secondary" onClick={onScan} disabled={scanning}><RefreshCw size={14} />{t("Scan", "扫描")}</Button>
    <Button variant="primary" onClick={onAdd}><Plus size={14} />{t("Add Skill", "新增 Skill")}</Button>
  </>}>
    <div className="grid grid-cols-1 gap-2 sm:grid-cols-[minmax(0,1fr)_10rem_10rem]">
      <SkTextInput type="search" value={filters.query} onChange={(query) => onFiltersChange({ ...filters, query })} placeholder={t("Search name, description, or source", "搜索名称、说明或来源")} aria-label={t("Search Skills", "搜索 Skills")} />
      <SkSelect value={filters.status} options={statusOptions} ariaLabel={t("Filter Skill status", "筛选 Skill 状态")} onChange={(status) => onFiltersChange({ ...filters, status })} />
      <SkSelect value={filters.scope} options={scopeOptions} ariaLabel={t("Filter Skill source", "筛选 Skill 来源")} onChange={(scope) => onFiltersChange({ ...filters, scope })} />
    </div>
    {error && <div className="settings-inline-error">{error}</div>}
    {scanning && <p role="status" className="text-xs">{t("Scanning Skill directories", "正在扫描 Skill 目录")}</p>}
    {!visibleSkills.length ? <EmptyGuide title={skills.length ? t("No matching Skills", "没有匹配的 Skill") : t("No Skills found", "尚未发现 Skill")} /> : layout === "grid" ? <ul className="skill-grid" aria-label={t("Available Skills", "可用 Skills")}>{visibleSkills.map((skill) => <SkillCard key={skill.id} skill={skill} onOpen={onOpen} />)}</ul> : <DataTable label={t("Available Skills", "可用 Skills")} rows={visibleSkills} rowKey={(skill) => skill.id} columns={[
      { id: "name", header: t("Name", "名称"), sortValue: (skill) => skill.name, render: (skill) => <Button variant="ghost" onClick={() => onOpen(skill.id)}>{skill.name}</Button> },
      { id: "description", header: t("Description", "说明"), render: (skill) => skill.description },
      { id: "source", header: t("Source", "来源"), sortValue: (skill) => skill.scope, render: (skill) => skillScopeLabel(skill.scope, t) },
      { id: "status", header: t("Status", "状态"), render: (skill) => <StatusBadge tone={skill.enabled ? "success" : "neutral"}>{skill.enabled ? t("Enabled", "已启用") : t("Disabled", "已禁用")}</StatusBadge> }
    ]} />}
  </SettingsPanel>;
}
