import type { AppConfig, RunMode } from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
import { buildChatModelChoices } from "../../chat/chat-model-options";
import { createRunModeOptions } from "../../permission/run-mode-options";
import { SettingsGroup } from "../editor-layout";
import { useI18n } from "../../i18n/use-i18n";

const SESSION_MODEL_VALUE = "";

type PermissionDefaultSettingsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染 TUI / CLI 默认权限模式，以及自动审核模型配置。
 *
 * @param props 应用配置和更新回调
 * @returns 默认权限模式设置分组
 */
export function PermissionDefaultSettings({ config, onConfigChange }: PermissionDefaultSettingsProps) {
  const { t } = useI18n();
  const tuiValue = config.permission?.tui_mode ?? config.permission?.default_mode ?? "yolo";
  const cliValue = config.permission?.cli_mode ?? config.permission?.default_mode ?? "yolo";
  const permissionOptions = createRunModeOptions(t);
  const autoProvider = config.permission?.auto_audit_provider_id ?? "";
  const autoModel = config.permission?.auto_audit_model ?? "";
  const autoAuditValue =
    autoProvider && autoModel ? encodeModelChoice(autoProvider, autoModel) : SESSION_MODEL_VALUE;
  const autoAuditOptions = [
    {
      value: SESSION_MODEL_VALUE,
      label: t("Session model", "会话模型"),
      description: t(
        "Use the model selected by the current conversation for each auto-audit.",
        "每次自动审核使用当前会话实际选择的模型。"
      )
    },
    ...buildChatModelChoices(config).map((choice) => ({
      value: encodeModelChoice(choice.providerId, choice.model),
      label: `${choice.providerName} / ${choice.model}`,
      description: t("Always use this model for automatic permission audits", "始终使用该模型进行自动权限审核")
    }))
  ];

  /**
   * 更新权限配置局部字段。
   *
   * @param patch 局部更新
   */
  const patchPermission = (patch: Partial<NonNullable<AppConfig["permission"]>>) => {
    onConfigChange({
      ...config,
      permission: {
        default_mode: config.permission?.default_mode ?? "yolo",
        tui_mode: config.permission?.tui_mode,
        cli_mode: config.permission?.cli_mode,
        auto_audit_provider_id: config.permission?.auto_audit_provider_id,
        auto_audit_model: config.permission?.auto_audit_model,
        ...patch
      }
    });
  };

  /** 更新 TUI 默认权限模式。 */
  const updateTuiMode = (mode: RunMode) => {
    patchPermission({
      default_mode: mode,
      tui_mode: mode,
      cli_mode: config.permission?.cli_mode ?? config.permission?.default_mode ?? "yolo"
    });
  };

  /** 更新 CLI 默认权限模式。 */
  const updateCliMode = (mode: RunMode) => {
    patchPermission({
      default_mode: config.permission?.tui_mode ?? config.permission?.default_mode ?? "yolo",
      tui_mode: config.permission?.tui_mode ?? config.permission?.default_mode ?? "yolo",
      cli_mode: mode
    });
  };

  /**
   * 更新自动审核模型（单段 供应商/模型 选择）。
   *
   * @param value 选择器编码值；空表示跟随会话模型
   */
  const updateAutoAuditModel = (value: string) => {
    const [providerId = "", model = ""] = value ? value.split("\u0000", 2) : [];
    patchPermission({
      auto_audit_provider_id: providerId || undefined,
      auto_audit_model: model || undefined
    });
  };

  return (
    <>
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
            <small>{t("Used by one-shot commands such as ask and tool when mode flags are omitted.", "ask / tool 等一次性命令未传模式参数时使用。")}</small>
          </div>
        </div>
      </SettingsGroup>
      <SettingsGroup title={t("Auto audit model", "自动审核模型")} description={t("Used only in Auto audit mode. Leave empty to reuse the current session model.", "仅自动审核模式使用。留空则沿用当前会话模型。")}>
        <div className="settings-form-grid">
          <label className="settings-field full">
            <span>{t("Provider / model", "供应商 / 模型")}</span>
            <Select
              value={autoAuditValue}
              options={autoAuditOptions}
              onChange={updateAutoAuditModel}
              ariaLabel={t("Auto audit model", "自动审核模型")}
              menuPreferredWidth={360}
              menuMinimumWidth={280}
            />
            <small>{t("An empty value follows the current conversation model.", "留空时自动跟随当前会话模型。")}</small>
          </label>
        </div>
      </SettingsGroup>
    </>
  );
}

/**
 * 编码供应商与模型为选择器值。
 *
 * @param providerId 供应商 id
 * @param model 模型 id
 * @returns 选择器内部编码
 */
function encodeModelChoice(providerId: string, model: string): string {
  return `${providerId}\u0000${model}`;
}
