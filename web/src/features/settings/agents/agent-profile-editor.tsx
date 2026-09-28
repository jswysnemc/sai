import { useSearchParams } from "react-router-dom";
import type { AppConfig } from "../../../api/contracts";
import type { AgentProfile } from "../../agents/agent-types";
import { DEFAULT_AGENT_ID } from "../../agents/agent-options";
import { useI18n } from "../../i18n/use-i18n";
import { DetailHeader, InlineSwitch, LocalTabs, SettingsPanel, StatusBadge, SwitchField } from "../kit";
import type { AgentOptions } from "./agents-types";
import { AgentSkillPermissions } from "./agent-skill-permissions";
import { AgentToolPermissions } from "./agent-tool-permissions";
import { AgentBasicsPanel } from "./agent-basics-panel";
import { BUILTIN_AGENT_PROFILES } from "./agent-profile-state";
import { DEFERRED_ALL_NON_BASE } from "./agent-tool-mode-state";
import { fieldAnchorId } from "../search/field-anchor";

type AgentEditorTab = "basic" | "tools" | "skills";
type Props = { config: AppConfig; profile: AgentProfile; options: AgentOptions; onChange: (patch: Partial<AgentProfile>) => void; onRemove: () => void };

/**
 * 【Agent】【档案编辑】组合对象操作、基础配置和能力权限页签。
 * @param props 配置、档案、能力选项与更新回调
 * @returns 档案编辑区
 */
export function AgentProfileEditor({ config, profile, options, onChange, onRemove }: Props) {
  const { t } = useI18n();
  const [params, setParams] = useSearchParams();
  const tab: AgentEditorTab = params.get("view") === "tools" ? "tools" : params.get("view") === "skills" ? "skills" : "basic";
  const skillCount = profile.skills_full.length + profile.skills_named.length;
  const deferred = profile.deferred_tools ?? [];
  const deferredCount = deferred.includes(DEFERRED_ALL_NON_BASE) ? options.tools.filter((tool) => !tool.resident).length : deferred.length;
  const builtin = profile.id === DEFAULT_AGENT_ID || BUILTIN_AGENT_PROFILES.some((item) => item.id === profile.id);
  return <>
    <DetailHeader title={profile.name || profile.id} badges={<>
      <StatusBadge>{t(`${profile.enabled_tools.length} tools`, `${profile.enabled_tools.length} 个工具`)}</StatusBadge>
      <StatusBadge>{t(`${deferredCount} on demand`, `${deferredCount} 个按需加载`)}</StatusBadge>
      <StatusBadge>{t(`${skillCount} Skills`, `${skillCount} 个技能`)}</StatusBadge>
    </>} actions={profile.id !== DEFAULT_AGENT_ID && <>
      <InlineSwitch label={t("Register with main Agent", "向主 Agent 注册")} checked={profile.register_to_main} onChange={(register_to_main) => onChange({ register_to_main })} />
      <InlineSwitch label={t("Load AGENT.md", "加载 AGENT.md")} checked={profile.load_instruction_files} onChange={(load_instruction_files) => onChange({ load_instruction_files })} />
    </>} menuItems={builtin ? [] : [{ id: "delete", label: t("Delete profile", "删除档案"), danger: true, onSelect: onRemove }]} />
    <LocalTabs value={tab} items={[{ id: "basic", label: t("Basics", "基础配置") }, { id: "tools", label: t("Tool permissions", "工具权限"), count: profile.enabled_tools.length }, { id: "skills", label: t("Skills", "技能"), count: skillCount }]} ariaLabel={t("Agent configuration categories", "Agent 配置分类")} onChange={(view) => setParams((current) => { const next = new URLSearchParams(current); next.set("view", view); return next; }, { replace: true })} />
    {tab === "basic" && <AgentBasicsPanel key={profile.id} config={config} profile={profile} options={options} onChange={onChange} />}
    {tab === "tools" && <SettingsPanel title={t("Tool whitelist", "工具白名单")} id={fieldAnchorId("agents.enabled_tools")}>
      <SwitchField label={t("Exclusive whitelist", "独占白名单")} checked={profile.tools_exclusive ?? false} hint={t("Only selected tools are available; an empty list disables every tool, including fallback tools.", "仅提供所选工具；留空会关闭全部工具，包括兜底工具。")} onChange={(tools_exclusive) => onChange({ tools_exclusive })} />
      <AgentToolPermissions tools={options.tools} enabled={profile.enabled_tools} deferred={deferred} onChange={(enabled_tools, deferred_tools) => onChange({ enabled_tools, deferred_tools })} />
    </SettingsPanel>}
    {tab === "skills" && <SettingsPanel title={t("Skill exposure", "技能暴露")} id={fieldAnchorId("agents.skills")}>
      <AgentSkillPermissions skills={options.skills} fullNames={profile.skills_full} namedNames={profile.skills_named} onChange={(skills_full, skills_named) => onChange({ skills_full, skills_named })} />
    </SettingsPanel>}
  </>;
}
