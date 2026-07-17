import { CheckCheck, Search, X } from "lucide-react";
import { useMemo, useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { Select } from "../../../shared/ui/select/select";
import type { AgentSkillOption } from "./agents-types";
import "./agent-permissions.css";

export type SkillPermission = "full" | "named" | "off";
type SkillStatusFilter = "all" | SkillPermission;

type AgentSkillPermissionsProps = {
  /** 全部可用 Skill */
  skills: AgentSkillOption[];
  /** 完整暴露的 Skill 名称 */
  fullNames: string[];
  /** 仅暴露名称的 Skill 名称 */
  namedNames: string[];
  /** Skill 权限变化回调 */
  onChange: (fullNames: string[], namedNames: string[]) => void;
};

const STATUS_FILTER_OPTIONS = [
  { value: "all", label: "全部状态" },
  { value: "full", label: "完整启用" },
  { value: "named", label: "仅名称" },
  { value: "off", label: "已关闭" }
] satisfies Array<{ value: SkillStatusFilter; label: string }>;

const PERMISSION_OPTIONS = [
  { value: "full", label: "完整启用", description: "暴露名称与完整说明" },
  { value: "named", label: "仅名称", description: "仅暴露 Skill 名称" },
  { value: "off", label: "关闭", description: "不向 Agent 暴露" }
] satisfies Array<{ value: SkillPermission; label: string; description: string }>;

/**
 * 更新单个 Skill 的三态权限，并保证两个输出数组互斥。
 *
 * @param fullNames 当前完整启用名称
 * @param namedNames 当前仅暴露名称
 * @param name 需要更新的 Skill 名称
 * @param permission 目标权限状态
 * @returns 更新后的 fullNames 与 namedNames
 */
export function updateSkillPermission(
  fullNames: string[],
  namedNames: string[],
  name: string,
  permission: SkillPermission
): { fullNames: string[]; namedNames: string[] } {
  const nextFull = fullNames.filter((item) => item !== name);
  const nextNamed = namedNames.filter((item) => item !== name);
  if (permission === "full") nextFull.push(name);
  if (permission === "named") nextNamed.push(name);
  return { fullNames: nextFull, namedNames: nextNamed };
}

/**
 * 渲染支持搜索、状态筛选和逐项三态设置的 Skill 权限面板。
 *
 * @param props Skill 列表、两类已启用名称和变化回调
 * @returns Skill 权限面板
 */
export function AgentSkillPermissions({ skills, fullNames, namedNames, onChange }: AgentSkillPermissionsProps) {
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<SkillStatusFilter>("all");

  /** 合并可用列表与配置中的未知 Skill，防止历史配置被静默隐藏。 */
  const allSkills = useMemo(() => {
    const known = new Map(skills.map((skill) => [skill.name, skill]));
    for (const name of [...fullNames, ...namedNames]) {
      if (!known.has(name)) known.set(name, { name, description: "当前环境未返回该 Skill 的说明" });
    }
    return [...known.values()];
  }, [fullNames, namedNames, skills]);

  /**
   * 获取指定 Skill 当前的权限状态。
   *
   * @param name Skill 名称
   * @returns 当前三态权限
   */
  const permissionOf = (name: string): SkillPermission => {
    if (fullNames.includes(name)) return "full";
    if (namedNames.includes(name)) return "named";
    return "off";
  };

  const normalizedQuery = query.trim().toLocaleLowerCase();
  const visibleSkills = allSkills.filter((skill) => {
    const matchesQuery = normalizedQuery.length === 0
      || skill.name.toLocaleLowerCase().includes(normalizedQuery)
      || skill.description.toLocaleLowerCase().includes(normalizedQuery);
    const matchesStatus = statusFilter === "all" || permissionOf(skill.name) === statusFilter;
    return matchesQuery && matchesStatus;
  });
  const enabledCount = new Set([...fullNames, ...namedNames]).size;

  return (
    <div className="agent-permissions-panel agent-skill-permissions">
      <div className="agent-permissions-toolbar">
        <label className="agent-permissions-search">
          <Search size={14} aria-hidden="true" />
          <input
            type="search"
            value={query}
            placeholder="搜索 Skill"
            aria-label="搜索 Skill"
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
        <div className="agent-skill-filter">
          <Select
            value={statusFilter}
            options={STATUS_FILTER_OPTIONS}
            ariaLabel="筛选 Skill 状态"
            menuMinimumWidth={144}
            onChange={setStatusFilter}
          />
        </div>
        <span className="agent-permissions-summary">已启用 {enabledCount}/{allSkills.length}</span>
        <div className="agent-permissions-actions">
          <Button onClick={() => onChange(allSkills.map((skill) => skill.name), [])}>
            <CheckCheck size={14} aria-hidden="true" />
            全部启用
          </Button>
          <Button onClick={() => onChange([], [])} disabled={enabledCount === 0}>
            <X size={14} aria-hidden="true" />
            全部关闭
          </Button>
        </div>
      </div>

      {allSkills.length === 0 ? (
        <p className="agent-permissions-empty">暂无可用 Skill。</p>
      ) : visibleSkills.length === 0 ? (
        <p className="agent-permissions-empty">没有匹配的 Skill。</p>
      ) : (
        <div className="agent-skill-permission-list">
          {visibleSkills.map((skill) => {
            const permission = permissionOf(skill.name);
            return (
              <article key={skill.name} className="agent-skill-permission-row" data-permission={permission}>
                <div className="agent-skill-permission-main">
                  <div className="agent-skill-permission-name">
                    <strong>{skill.name}</strong>
                    <em data-permission={permission}>
                      {permission === "full" ? "完整" : permission === "named" ? "名称" : "关闭"}
                    </em>
                  </div>
                  {skill.description && <span>{skill.description}</span>}
                </div>
                <div className="agent-skill-permission-select">
                  <Select
                    value={permission}
                    options={PERMISSION_OPTIONS}
                    ariaLabel={`设置 ${skill.name} 的权限`}
                    menuPreferredWidth={208}
                    menuAlign="right"
                    onChange={(nextPermission) => {
                      const next = updateSkillPermission(fullNames, namedNames, skill.name, nextPermission);
                      onChange(next.fullNames, next.namedNames);
                    }}
                  />
                </div>
              </article>
            );
          })}
        </div>
      )}
    </div>
  );
}
