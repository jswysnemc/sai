import { Bot } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { AgentProfileConfig, AppConfig } from "../../../api/contracts";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { DEFAULT_AGENT_ID } from "../../agents/agent-options";
import type { AgentProfile } from "../../agents/agent-types";
import { useI18n } from "../../i18n/use-i18n";
import { ObjectListPanel } from "../object-list-panel";
import { AgentProfileEditor } from "./agent-profile-editor";
import {
  BUILTIN_AGENT_PROFILES,
  buildVisibleAgentProfiles,
  createUniqueAgentProfile,
  removeAgentProfile,
  updateAgentProfile
} from "./agent-profile-state";
import type { AgentOptions } from "./agents-types";

type AgentProfileWorkspaceProps = {
  config: AppConfig;
  options: AgentOptions;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 组合主 Agent 档案列表与当前档案编辑器。
 *
 * @param props 应用配置、可选能力和配置更新回调
 * @returns 主 Agent 档案工作区
 */
export function AgentProfileWorkspace({ config, options, onConfigChange }: AgentProfileWorkspaceProps) {
  const confirm = useConfirm();
  const { locale, t } = useI18n();
  const stored = config.agents ?? [];
  const profiles = useMemo(
    () => buildVisibleAgentProfiles(stored, options, config.subagent?.profiles, locale),
    [config.subagent?.profiles, locale, options, stored]
  );
  const [selectedId, setSelectedId] = useState(DEFAULT_AGENT_ID);
  const selected = profiles.find((profile) => profile.id === selectedId) ?? profiles[0] ?? null;

  useEffect(() => {
    if (selected && selected.id !== selectedId) setSelectedId(selected.id);
  }, [selected, selectedId]);

  /**
   * 写回主 Agent 档案数组。
   *
   * @param next 更新后的档案数组
   * @returns 无返回值
   */
  const persistProfiles = (next: AgentProfileConfig[]) => {
    onConfigChange({ ...config, agents: next });
  };

  /**
   * 新增启用全部当前工具和 Skills 的档案。
   *
   * @returns 无返回值
  */
  const addProfile = () => {
    // 1. 生成不与现有标识冲突的新档案
    const created = createUniqueAgentProfile(stored, options, {}, locale);
    // 2. 写回配置并切换到新档案
    persistProfiles([...stored, created]);
    setSelectedId(created.id);
  };

  /**
   * 更新当前档案，虚拟默认档案首次修改时写入配置。
   *
   * @param patch 需要覆盖的档案字段
   * @returns 无返回值
  */
  const updateSelected = (patch: Partial<AgentProfile>) => {
    if (!selected) return;
    // 1. 判断当前档案是否已经持久化
    const exists = stored.some((profile) => profile.id === selected.id);
    // 2. 更新已有档案，或将首次修改的虚拟默认档案写入配置
    persistProfiles(exists
      ? updateAgentProfile(stored, selected.id, patch)
      : [{ ...selected, ...patch }, ...stored]);
  };

  /**
   * 确认并删除当前自定义档案。
   *
   * @returns 删除流程完成后的 Promise
   */
  const removeSelected = async () => {
    if (!selected || selected.id === DEFAULT_AGENT_ID || BUILTIN_AGENT_PROFILES.some((profile) => profile.id === selected.id)) return;
    // 1. 使用统一确认对话框核对删除操作
    const confirmed = await confirm({
      title: t("Delete Agent", "删除 Agent"),
      description: t(`Delete all configuration for “${selected.name || selected.id}”.`, `将删除“${selected.name || selected.id}”的全部配置。`),
      confirmLabel: t("Delete Agent", "删除 Agent"),
      danger: true
    });
    if (!confirmed) return;
    // 2. 删除档案并切换到剩余的可用档案
    const next = removeAgentProfile(stored, selected.id);
    persistProfiles(next);
    setSelectedId(next[0]?.id ?? DEFAULT_AGENT_ID);
  };

  return (
    <div className="settings-objects-layout agent-profile-workspace">
      <ObjectListPanel
        title={t("Agent profiles", "Agent 档案")}
        items={profiles.map((profile) => {
          const skillCount = profile.skills_full.length + profile.skills_named.length;
          const badges: string[] = [];
          if (profile.id === DEFAULT_AGENT_ID) badges.push(t("Default", "默认"));
          else if (["general", "explore"].includes(profile.id)) badges.push(t("Built-in", "内置"));
          if (profile.register_to_main && profile.id !== DEFAULT_AGENT_ID) badges.push(t("Registered", "已注册"));
          const metaParts = [
            t(`${profile.enabled_tools.length} tools`, `${profile.enabled_tools.length} 工具`),
            `${skillCount} Skills`,
            profile.model ? profile.model : t("Inherit model", "沿用模型")
          ];
          if (badges.length > 0) metaParts.unshift(badges.join(" · "));
          return {
            id: profile.id,
            name: profile.name || profile.id,
            meta: metaParts.join(" · "),
            icon: <Bot size={14} />,
            marked: profile.id === DEFAULT_AGENT_ID || profile.register_to_main
          };
        })}
        selectedId={selected?.id ?? ""}
        searchPlaceholder={t("Search Agents", "搜索 Agent")}
        addLabel={t("Add Agent", "新增 Agent")}
        onSelect={setSelectedId}
        onAdd={addProfile}
      />
      {selected ? (
        <AgentProfileEditor
          config={config}
          profile={selected}
          options={options}
          onChange={updateSelected}
          onRemove={() => void removeSelected()}
        />
      ) : (
        <div className="settings-empty">{t("No editable Agent profiles", "没有可编辑的 Agent 档案")}</div>
      )}
    </div>
  );
}
