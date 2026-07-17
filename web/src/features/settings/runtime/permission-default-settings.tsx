import type { AppConfig, RunMode } from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
import { SettingsGroup } from "../editor-layout";

const PERMISSION_OPTIONS = [
  { value: "audited", label: "审计", description: "写入工具逐次询问，并限制在工作区沙盒内。" },
  { value: "plan", label: "规划", description: "仅允许只读工具，禁止修改文件和执行写操作。" },
  { value: "yolo", label: "YOLO", description: "不询问工具权限，直接执行允许的工具。" }
] satisfies Array<{ value: RunMode; label: string; description: string }>;

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
  const tuiValue = config.permission?.tui_mode ?? config.permission?.default_mode ?? "yolo";
  const cliValue = config.permission?.cli_mode ?? config.permission?.default_mode ?? "yolo";

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
    <SettingsGroup title="默认权限" description="TUI 与 CLI 可分别配置默认权限模式；命令行参数仍可临时覆盖。">
      <div className="settings-form-grid">
        <div className="settings-field">
          <span>TUI 默认模式</span>
          <Select value={tuiValue} options={PERMISSION_OPTIONS} onChange={updateTuiMode} ariaLabel="TUI 默认权限模式" menuPreferredWidth={330} />
          <small>交互式 REPL / 终端界面未传模式参数时使用。</small>
        </div>
        <div className="settings-field">
          <span>CLI 默认模式</span>
          <Select value={cliValue} options={PERMISSION_OPTIONS} onChange={updateCliMode} ariaLabel="CLI 默认权限模式" menuPreferredWidth={330} />
          <small>ask / tool 等一次性命令未传 --yolo/--audited/--plan 时使用。</small>
        </div>
      </div>
    </SettingsGroup>
  );
}
