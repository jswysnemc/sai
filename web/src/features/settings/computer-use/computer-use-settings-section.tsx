import type { AppConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import {
  FieldGrid,
  InlineNotice,
  SettingsField,
  SettingsPanel,
  SkNumberInput,
  SkTextInput,
  SwitchField
} from "../kit";
import { ComputerUseDriverPanel } from "./computer-use-driver-panel";
import { readComputerUse, shouldReplaceDriverPath, writeComputerUse } from "./computer-use-config";
import "./computer-use-settings.css";

type ComputerUseSettingsSectionProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 【桌面操作】【设置分区】编辑开关、驱动路径和超时，并提供本机驱动安装。
 *
 * @param props 应用配置与更新回调
 * @returns 桌面操作设置
 */
export function ComputerUseSettingsSection({ config, onConfigChange }: ComputerUseSettingsSectionProps) {
  const { t } = useI18n();
  const computerUse = readComputerUse(config);

  /**
   * 把安装得到的路径写回草稿；自定义路径保持不变。
   *
   * @param path 安装后的绝对路径
   * @returns 无返回值
   */
  const applyInstalledPath = (path: string) => {
    if (!shouldReplaceDriverPath(computerUse.driver_path, path)) return;
    onConfigChange(writeComputerUse(config, { driver_path: path }));
  };

  return (
    <div className="computer-use-settings">
      <InlineNotice tone="info">
        {t(
          "Computer Use drives native apps on the host that runs Sai, not the built-in browser. Installing the driver does not enable the feature or grant OS permissions.",
          "桌面操作控制的是运行 Sai 的主机上的原生应用，不是内置浏览器。安装驱动不会开启功能，也不会授予系统权限。"
        )}
      </InlineNotice>
      <SwitchField
        label={t("Enable Computer Use", "启用桌面操作")}
        hint={t("Required before the computer_use tool can start cua-driver. Keep this off unless you need native desktop control.", "开启后 computer_use 工具才能启动 cua-driver。不需要原生桌面控制时保持关闭。")}
        configKey="computer_use.enabled"
        anchor="computer-use.enabled"
        checked={computerUse.enabled}
        onChange={(enabled) => onConfigChange(writeComputerUse(config, { enabled }))}
      />
      <SettingsPanel
        title={t("Driver", "驱动")}
        description={t("Install the official cua-driver package, then save the path with the top bar.", "安装官方 cua-driver 后，用顶栏保存路径。")}
      >
        <ComputerUseDriverPanel driverPath={computerUse.driver_path} onInstalledPath={applyInstalledPath} />
        <FieldGrid>
          <SettingsField
            label={t("Driver path", "驱动路径")}
            hint={t("Executable name or absolute path. Sai runs this file directly, not through a shell.", "可执行文件名或绝对路径。Sai 直接执行该文件，不经过 Shell。")}
            configKey="computer_use.driver_path"
            anchor="computer-use.driver_path"
            span="full"
          >
            <SkTextInput
              mono
              value={computerUse.driver_path}
              onChange={(driver_path) => onConfigChange(writeComputerUse(config, { driver_path }))}
            />
          </SettingsField>
          <SettingsField
            label={t("Call timeout", "调用超时")}
            hint={t("Covers initialize, queue, and the current native request.", "覆盖初始化、排队和本次原生请求。")}
            configKey="computer_use.timeout_ms"
            anchor="computer-use.timeout_ms"
            size="sm"
          >
            <SkNumberInput
              value={computerUse.timeout_ms}
              min={1_000}
              max={120_000}
              integer
              step={1_000}
              unit="ms"
              onChange={(value) => onConfigChange(writeComputerUse(config, { timeout_ms: value ?? 30_000 }))}
            />
          </SettingsField>
        </FieldGrid>
      </SettingsPanel>
      <InlineNotice tone="info">
        {t(
          "After enabling, grant Accessibility and screen recording on macOS, or AT-SPI on Linux, using the driver documentation. Sai never grants those permissions itself.",
          "启用后请按驱动说明在 macOS 授予辅助功能与屏幕录制，或在 Linux 配置 AT-SPI。Sai 不会自行授予这些权限。"
        )}
      </InlineNotice>
    </div>
  );
}
