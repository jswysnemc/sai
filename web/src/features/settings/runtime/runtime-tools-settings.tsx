import type { DisplayConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsPanel, SwitchField } from "../kit";
import { RtkFilterSettings } from "../rtk-filter-settings";
import { StructuredConfigFields } from "../structured-config-fields";
import { ContextSettings } from "./context-settings";
import { DebugSettings } from "./debug-settings";
import type { RuntimeSettingsProps } from "./runtime-settings-types";

/** 终端专属显示字段，改到「终端与输入」里编辑，避免在输出显示重复出现。 */
const TUI_DISPLAY_KEYS = [
  "fullscreen",
  "math_images",
  "mermaid_images",
  "fold_preview",
  "fold_head_lines",
  "fold_tail_lines"
] as const;

/**
 * 从显示配置中去掉终端专属字段，供输出显示面板编辑。
 *
 * @param display 完整显示配置
 * @returns 不含终端专属键的对象
 */
function displayForOutputPanel(display: DisplayConfig): Record<string, unknown> {
  const next: Record<string, unknown> = { ...display };
  for (const key of TUI_DISPLAY_KEYS) delete next[key];
  return next;
}

/**
 * 【Web 设置】【工具与上下文】组合上下文预算、工具执行、输出过滤和调试设置。
 * @param props 应用配置与更新回调
 * @returns 工具与上下文设置子页
 */
export function RuntimeToolsSettings({ config, onConfigChange }: RuntimeSettingsProps) {
  const { t } = useI18n();
  const { command_filter: _filter, command_filter_denylist: _denylist, max_rounds: _rounds, ...tools } = config.tools ?? {};
  return (
    <>
      <ContextSettings config={config} onConfigChange={onConfigChange} />
      <SettingsPanel title={t("Tool execution", "工具执行")} description={t("Control tool availability, shell commands, and background execution.", "控制工具可用性、Shell 命令与后台执行。")}>
        <StructuredConfigFields value={tools} configPath="tools" anchorPrefix="runtime.tools" onChange={(next) => onConfigChange({ ...config, tools: { ...config.tools, ...next } })} />
      </SettingsPanel>
      <SettingsPanel title={t("Command output filter (rtk)", "命令输出过滤器（rtk）")}>
        <RtkFilterSettings config={config} onConfigChange={onConfigChange} />
      </SettingsPanel>
      <SettingsPanel title={t("Output display", "输出显示")}>
        <StructuredConfigFields value={displayForOutputPanel(config.display ?? {})} configPath="display" anchorPrefix="runtime.display" onChange={(next) => {
          const display = { ...config.display };
          for (const key of TUI_DISPLAY_KEYS) {
            if (display[key] !== undefined) next[key] = display[key];
          }
          onConfigChange({ ...config, display: next });
        }} />
      </SettingsPanel>
      <SettingsPanel title={t("Session mesh", "会话网格")}>
        <SwitchField label={t("Cross-session messaging", "跨会话投递")} hint={t("Allow mesh messages to other sessions; disabled by default.", "允许向其他会话发送网格消息，默认关闭。")} configKey="mesh.cross_session" anchor="runtime.mesh.cross_session" checked={config.mesh?.cross_session ?? false} onChange={(checked) => onConfigChange({ ...config, mesh: { ...config.mesh, cross_session: checked } })} />
      </SettingsPanel>
      <DebugSettings config={config} onConfigChange={onConfigChange} />
    </>
  );
}
