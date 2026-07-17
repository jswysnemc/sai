import type { AppConfig, TerminalConfig } from "../../api/contracts";

type TerminalSettingsFieldsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染网页终端 Shell 配置。
 *
 * @param props 应用配置与更新回调
 * @returns 网页终端配置字段
 */
export function TerminalSettingsFields({ config, onConfigChange }: TerminalSettingsFieldsProps) {
  const terminal: TerminalConfig = config.terminal ?? { shell: "" };

  return (
    <div className="settings-form-grid">
      <label className="settings-field full">
        <span>终端 Shell</span>
        <input
          type="text"
          value={terminal.shell}
          placeholder="留空使用平台默认 Shell"
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => onConfigChange({
            ...config,
            terminal: { ...terminal, shell: event.target.value }
          })}
        />
        <small>填写可执行文件路径或名称，不包含启动参数。Unix 留空使用用户登录 Shell，Windows 留空使用 PowerShell。</small>
      </label>
    </div>
  );
}
