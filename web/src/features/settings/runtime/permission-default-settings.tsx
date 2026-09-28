import type { AppConfig, RunMode } from "../../../api/contracts";
import { createRunModeOptions } from "../../permission/run-mode-options";
import { useI18n } from "../../i18n/use-i18n";
import { ModelChoiceSelect } from "../controls/model-choice-select";
import { FieldGrid, SettingsField, SettingsPanel, SkSelect } from "../kit";
import type { RuntimeSettingsProps } from "./runtime-settings-types";

/**
 * 【Web 设置】【权限默认值】配置终端界面、单次命令的权限模式与审核模型。
 * @param props 应用配置与更新回调
 * @returns 权限设置面板
 */
export function PermissionDefaultSettings({ config, onConfigChange }: RuntimeSettingsProps) {
  const { t } = useI18n();
  const permission = config.permission;
  const tuiMode = permission?.tui_mode ?? permission?.default_mode ?? "yolo";
  const cliMode = permission?.cli_mode ?? permission?.default_mode ?? "yolo";
  const options = createRunModeOptions(t);

  /**
   * 合并权限补丁，保留审核插件等未展示字段。
   * @param patch 修改的权限字段
   * @returns 无返回值
   */
  const update = (patch: Partial<NonNullable<AppConfig["permission"]>>) => onConfigChange({
    ...config,
    permission: { ...permission, default_mode: tuiMode, ...patch }
  });

  /**
   * 同步终端界面与旧版默认权限字段。
   * @param mode 选中的权限模式
   * @returns 无返回值
   */
  const updateTuiMode = (mode: RunMode) => update({ default_mode: mode, tui_mode: mode, cli_mode: cliMode });

  return (
    <SettingsPanel title={t("Default permissions", "默认权限")} description={t("TUI and CLI can use separate modes; command-line options can override them for a single run.", "TUI 与 CLI 可分别配置默认权限；命令行参数可临时覆盖。")}>
      <FieldGrid>
        <SettingsField label={t("TUI default mode", "TUI 默认模式")} hint={t("Used by the interactive terminal when no mode is specified.", "交互式终端未指定模式时使用。")} anchor="runtime.permission.tui_mode" configKey="permission.tui_mode" size="md">
          <SkSelect value={tuiMode} options={options} onChange={updateTuiMode} menuPreferredWidth={330} menuClassName="run-mode-menu" />
        </SettingsField>
        <SettingsField label={t("CLI default mode", "CLI 默认模式")} hint={t("Used by one-shot ask and tool commands when no mode is specified.", "ask、tool 等单次命令未指定模式时使用。")} anchor="runtime.permission.cli_mode" configKey="permission.cli_mode" size="md">
          <SkSelect value={cliMode} options={options} onChange={(mode) => update({ tui_mode: tuiMode, cli_mode: mode })} menuPreferredWidth={330} menuClassName="run-mode-menu" />
        </SettingsField>
        <SettingsField label={t("Auto audit model", "自动审核模型")} hint={config.jev?.audit?.enabled ? t("Jev audit is enabled, so this model is not used.", "已启用 Jev 审核，此模型不会用于审核。") : t("Used in Auto audit mode. Leave empty to follow the current conversation.", "自动审核模式使用；留空则跟随当前会话模型。")} anchor="runtime.permission.auto_audit_model" configKey="permission.auto_audit_model">
          <ModelChoiceSelect config={config} providerId={permission?.auto_audit_provider_id} model={permission?.auto_audit_model} inheritLabel={t("Session model", "会话模型")} onChange={(providerId, model) => update({ auto_audit_provider_id: providerId || undefined, auto_audit_model: model || undefined })} />
        </SettingsField>
      </FieldGrid>
    </SettingsPanel>
  );
}
