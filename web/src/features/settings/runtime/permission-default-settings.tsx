import type { AppConfig, RunMode } from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
import { createRunModeOptions } from "../../permission/run-mode-options";
import { SettingsGroup } from "../editor-layout";
import { useI18n } from "../../i18n/use-i18n";

type PermissionDefaultSettingsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染 TUI / CLI 各自的默认权限模式设置。
 *
 * @param props 应用配置和更新回调
 * @returns 默认权限模式设置分组
 */
export function PermissionDefaultSettings({ config, onConfigChange }: PermissionDefaultSettingsProps) {
  const { t } = useI18n();
  const tuiValue = config.permission?.tui_mode ?? config.permission?.default_mode ?? "yolo";
  const cliValue = config.permission?.cli_mode ?? config.permission?.default_mode ?? "yolo";
  const permissionOptions = createRunModeOptions(t);

  /**
   * 更新 TUI 默认权限模式。
   *
   * @param mode 新默认权限模式
   */
  const updateTuiMode = (mode: RunMode) => {
    onConfigChange({
      ...config,
      permission: {
        default_mode: mode,
        tui_mode: mode,
        cli_mode: config.permission?.cli_mode ?? config.permission?.default_mode ?? "yolo"
      }
    });
  };

  /**
   * 更新 CLI 默认权限模式。
   *
   * @param mode 新默认权限模式
   */
  const updateCliMode = (mode: RunMode) => {
    onConfigChange({
      ...config,
      permission: {
        default_mode: config.permission?.tui_mode ?? config.permission?.default_mode ?? "yolo",
        tui_mode: config.permission?.tui_mode ?? config.permission?.default_mode ?? "yolo",
        cli_mode: mode
      }
    });
  };

  return (
    <SettingsGroup title={t("Default permissions", "默认权限")} description={t("TUI and CLI can use separate default permission modes; command-line options can still override them temporarily.", "TUI 与 CLI 可分别配置默认权限模式；命令行参数仍可临时覆盖。")}>
      <div className="settings-form-grid">
        <div className="settings-field">
          <span>{t("TUI default mode", "TUI 默认模式")}</span>
          <Select value={tuiValue} options={permissionOptions} onChange={updateTuiMode} ariaLabel={t("TUI default permission mode", "TUI 默认权限模式")} menuPreferredWidth={330} />
          <small>{t("Used by the interactive REPL and terminal interface when no mode option is provided.", "交互式 REPL / 终端界面未传模式参数时使用。")}</small>
        </div>
        <div className="settings-field">
          <span>{t("CLI default mode", "CLI 默认模式")}</span>
          <Select value={cliValue} options={permissionOptions} onChange={updateCliMode} ariaLabel={t("CLI default permission mode", "CLI 默认权限模式")} menuPreferredWidth={330} />
          <small>{t("Used by one-shot commands such as ask and tool when --yolo, --audited, or --plan is omitted.", "ask / tool 等一次性命令未传 --yolo/--audited/--plan 时使用。")}</small>
        </div>
      </div>
    </SettingsGroup>
  );
}
