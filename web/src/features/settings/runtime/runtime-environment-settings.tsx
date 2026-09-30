import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsPanel } from "../kit";
import { TerminalSettingsFields } from "../terminal-settings-fields";
import { PasteKeySettings } from "./paste-key-settings";
import { PermissionDefaultSettings } from "./permission-default-settings";
import { SandboxSettings } from "./sandbox/sandbox-settings";
import { TerminalDisplaySettings } from "./terminal-display-settings";
import type { RuntimeSettingsProps } from "./runtime-settings-types";

/**
 * 【Web 设置】【环境与权限】组合权限默认值、命令沙箱、网页终端与终端界面输入配置。
 * @param props 应用配置与更新回调
 * @returns 环境与权限设置子页
 */
export function RuntimeEnvironmentSettings(props: RuntimeSettingsProps) {
  const { t } = useI18n();
  return (
    <>
      <PermissionDefaultSettings {...props} />
      <SandboxSettings {...props} />
      <SettingsPanel title={t("Terminal and input", "终端与输入")} description={t("The Web terminal uses the configured shell; the paste shortcut applies to the terminal UI.", "网页终端使用所选 Shell；粘贴键位用于终端界面输入框。")}>
        <FieldGrid>
          <TerminalSettingsFields {...props} />
          <PasteKeySettings {...props} />
        </FieldGrid>
      </SettingsPanel>
      <TerminalDisplaySettings />
    </>
  );
}
