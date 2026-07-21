import { useEffect, useState } from "react";
import { Trash2, Wrench, Sparkles, Settings2 } from "lucide-react";
import type { AppConfig } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { TextArea } from "../../../shared/ui/form/text-area";
import type { AgentProfile } from "../../agents/agent-types";
import { DEFAULT_AGENT_ID } from "../../agents/agent-options";
import { useI18n } from "../../i18n/use-i18n";
import { EditorHeader } from "../editor-layout";
import type { AgentOptions } from "./agents-types";
import { AgentSkillPermissions } from "./agent-skill-permissions";
import { AgentToolPermissions } from "./agent-tool-permissions";
import { AgentRuntimeFields } from "./agent-runtime-fields";

type AgentEditorTab = "basic" | "tools" | "skills";

type AgentProfileEditorProps = {
  config: AppConfig;
  profile: AgentProfile;
  options: AgentOptions;
  onChange: (patch: Partial<AgentProfile>) => void;
  onRemove: () => void;
};

/**
 * 渲染主 Agent 档案编辑器，并按基础配置、工具权限和 Skills 分页。
 *
 * @param props 当前配置、档案、可选能力和操作回调
 * @returns 主 Agent 档案编辑器
 */
export function AgentProfileEditor({ config, profile, options, onChange, onRemove }: AgentProfileEditorProps) {
  const { t } = useI18n();
  const [tab, setTab] = useState<AgentEditorTab>("basic");
  const skillCount = profile.skills_full.length + profile.skills_named.length;
  const isBuiltin = profile.id === DEFAULT_AGENT_ID || ["general", "explore"].includes(profile.id);
  const tabs: Array<{ id: AgentEditorTab; label: string; icon: typeof Settings2 }> = [
    { id: "basic", label: t("Basics", "基础配置"), icon: Settings2 },
    { id: "tools", label: t("Tool permissions", "工具权限"), icon: Wrench },
    { id: "skills", label: t("Skills", "技能"), icon: Sparkles }
  ];

  useEffect(() => {
    setTab("basic");
  }, [profile.id]);

  return (
    <section className="settings-editor agent-profile-editor">
      <EditorHeader
        kicker="Agent"
        title={profile.name || profile.id}
        description={
          profile.description
            ? `${profile.id} · ${profile.description}`
            : t(`${profile.id}, with ${profile.enabled_tools.length} tools and ${skillCount} Skills enabled.`, `${profile.id}，已启用 ${profile.enabled_tools.length} 个工具和 ${skillCount} 个 Skills。`)
        }
        actions={<>
          {profile.id !== DEFAULT_AGENT_ID && (
            <label className="settings-switch">
              <input
                type="checkbox"
                checked={profile.register_to_main}
                onChange={(event) => onChange({ register_to_main: event.target.checked })}
              />
              <span />
              <strong>{profile.register_to_main ? t("Registered with main Agent", "已向主 Agent 注册") : t("Not registered", "未注册")}</strong>
            </label>
            <label className="agent-toggle">
              <input
                type="checkbox"
                checked={profile.load_instruction_files}
                onChange={(event) => onChange({ load_instruction_files: event.target.checked })}
              />
              <span>
                <strong>{profile.load_instruction_files ? t("Loads AGENT.md files", "加载 AGENT.md") : t("Skips AGENT.md files", "不加载 AGENT.md")}</strong>
                <small>{t("Global and project instruction files", "全局与项目指令文件")}</small>
              </span>
            </label>

          )}
          {!isBuiltin && (
            <Button className="settings-danger" onClick={onRemove}>
              <Trash2 size={14} />{t("Delete profile", "删除档案")}
            </Button>
          )}
        </>}
      />

      <div className="agent-profile-stats" aria-label={t("Profile summary", "档案摘要")}>
        <div>
          <span>{t("Tools", "工具")}</span>
          <strong>{profile.enabled_tools.length}</strong>
          <small>/{options.tools.length || "—"}</small>
        </div>
        <div>
          <span>{t("Skills", "技能")}</span>
          <strong>{skillCount}</strong>
          <small>/{options.skills.length || "—"}</small>
        </div>
        <div>
          <span>{t("Model", "模型")}</span>
          <strong>{profile.model ? `${profile.provider_id || "?"} / ${profile.model}` : t("Inherit current", "沿用当前")}</strong>
        </div>
        <div>
          <span>{t("Thinking", "思考")}</span>
          <strong>{profile.thinking_level || "auto"}</strong>
        </div>
      </div>

      <nav className="settings-tabs agent-editor-tabs" aria-label={t("Agent configuration categories", "Agent 配置分类")}>
        {tabs.map(({ id, label, icon: Icon }) => (
          <Button
            key={id}
            className={tab === id ? "active" : ""}
            onClick={() => setTab(id)}
          >
            <Icon size={14} aria-hidden="true" />
            {label}
            {id === "tools" && <em>{profile.enabled_tools.length}</em>}
            {id === "skills" && <em>{skillCount}</em>}
          </Button>
        ))}
      </nav>

      {tab === "basic" && (
        <div className="agent-basic-form">
          <div className="settings-form-grid">
            <label className="settings-field">
              <span>{t("Display name", "显示名称")}</span>
              <input value={profile.name} onChange={(event) => onChange({ name: event.target.value })} />
              <small>{t("Used in Agent selection menus and runtime status", "用于 Agent 选择菜单和运行状态展示")}</small>
            </label>
            <AgentRuntimeFields
              config={config}
              providerId={profile.provider_id}
              model={profile.model}
              thinkingLevel={profile.thinking_level}
              inheritModelLabel={t("Inherit current model", "沿用当前模型")}
              thinkingHelp={t("Override the provider's default reasoning effort", "覆盖供应商的默认推理强度")}
              onChange={onChange}
            />
            <label className="settings-field full">
              <span>{t("Purpose", "用途描述")}</span>
              <input value={profile.description} onChange={(event) => onChange({ description: event.target.value })} />
              <small>{t("The main Agent uses this description to decide when to invoke this Agent", "主 Agent 根据这段描述判断是否调用该 Agent")}</small>
            </label>
          </div>
          <label className="settings-field agent-prompt-field">
            <span>{t("System prompt", "系统提示词")}</span>
            <TextArea
              value={profile.system_prompt}
              onChange={(event) => onChange({ system_prompt: event.target.value })}
              placeholder={t("Describe responsibilities, boundaries, and output requirements", "描述职责、边界和输出要求")}
            />
            <small>{t("Keep only stable role constraints here. Conversation input still provides the specific task.", "只写长期稳定的角色约束，具体任务仍由会话输入提供。")}</small>
          </label>
        </div>
      )}
      {tab === "tools" && (
        <AgentToolPermissions
          tools={options.tools}
          enabled={profile.enabled_tools}
          onChange={(enabledTools) => onChange({ enabled_tools: enabledTools })}
        />
      )}
      {tab === "skills" && (
        <AgentSkillPermissions
          skills={options.skills}
          fullNames={profile.skills_full}
          namedNames={profile.skills_named}
          onChange={(fullNames, namedNames) => onChange({ skills_full: fullNames, skills_named: namedNames })}
        />
      )}
    </section>
  );
}
