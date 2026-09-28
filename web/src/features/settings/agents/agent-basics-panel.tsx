import { useState } from "react";
import { Pencil } from "../../../shared/ui/icons";
import { Button } from "../../../shared/ui/button/button";
import type { AppConfig } from "../../../api/contracts";
import type { AgentProfile } from "../../agents/agent-types";
import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsField, SettingsPanel, SkTextInput } from "../kit";
import type { AgentOptions } from "./agents-types";
import { AgentRuntimeFields } from "./agent-runtime-fields";
import { AgentPromptSections } from "./agent-prompt-sections";
import { AgentPromptEditorDialog } from "./agent-prompt-editor-dialog";
import { promptPreviewText } from "./prompt-preview-text";

type Props = { config: AppConfig; profile: AgentProfile; options: AgentOptions; onChange: (patch: Partial<AgentProfile>) => void };

/**
 * 【Agent】【基础配置】编辑身份、模型和提示词，预览与原始文本独立。
 * @param props 配置、当前档案、选项与草稿更新回调
 * @returns 基础配置面板
 */
export function AgentBasicsPanel({ config, profile, options, onChange }: Props) {
  const { t } = useI18n();
  const [promptOpen, setPromptOpen] = useState(false);
  return <>
    <SettingsPanel title={t("Identity and runtime", "身份与运行时")}>
      <FieldGrid>
        <SettingsField label={t("Display name", "显示名称")} anchor="agents.name" hint={t("Used in selection menus and runtime status.", "用于选择菜单和运行状态展示。")}><SkTextInput value={profile.name} onChange={(name) => onChange({ name })} /></SettingsField>
        <AgentRuntimeFields config={config} providerId={profile.provider_id} model={profile.model} thinkingLevel={profile.thinking_level} inheritModelLabel={t("Inherit current model", "沿用当前模型")} thinkingHelp={t("Override the provider's default reasoning effort.", "覆盖供应商的默认推理强度。")} onChange={onChange} />
        <SettingsField label={t("Purpose", "用途描述")} anchor="agents.description" span="full" hint={t("The main Agent uses this to decide when to invoke this profile.", "主 Agent 根据这段描述判断何时调用该档案。")}><SkTextInput value={profile.description} onChange={(description) => onChange({ description })} /></SettingsField>
      </FieldGrid>
    </SettingsPanel>
    <SettingsPanel title={t("System prompt", "系统提示词")} actions={<Button variant="secondary" onClick={() => setPromptOpen(true)}><Pencil size={14} />{t("Edit prompt", "编辑提示词")}</Button>}>
      <SettingsField label={t("Preview", "预览")} anchor="agents.system_prompt" hint={t(`${profile.system_prompt.length} characters configured`, `已配置 ${profile.system_prompt.length} 字符`)}>
        <div className="agent-prompt-preview">{promptPreviewText(profile.system_prompt) || t("No system prompt yet.", "尚未配置系统提示词。")}</div>
      </SettingsField>
      <AgentPromptEditorDialog open={promptOpen} value={profile.system_prompt} onClose={() => setPromptOpen(false)} onApply={(system_prompt) => onChange({ system_prompt })} />
      <AgentPromptSections sections={profile.prompt_sections} options={options.prompt_sections} onChange={(prompt_sections) => onChange({ prompt_sections })} />
    </SettingsPanel>
  </>;
}
