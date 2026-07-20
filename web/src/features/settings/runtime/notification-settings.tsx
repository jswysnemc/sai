import type { AppConfig } from "../../../api/contracts";
import { SettingsGroup } from "../editor-layout";
import { useI18n } from "../../i18n/use-i18n";

type NotificationSettingsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 配置 TUI / Web 答复完成通知；CLI 不使用此项。
 *
 * @param props 应用配置与更新回调
 * @returns 通知设置分组
 */
export function NotificationSettings({ config, onConfigChange }: NotificationSettingsProps) {
  const { t } = useI18n();
  const enabled = config.notification?.enabled ?? true;
  const sound = config.notification?.sound ?? true;

  /**
   * 更新通知配置局部字段。
   *
   * @param patch 局部更新
   */
  const patch = (patch: { enabled?: boolean; sound?: boolean }) => {
    onConfigChange({
      ...config,
      notification: {
        enabled: patch.enabled ?? enabled,
        sound: patch.sound ?? sound
      }
    });
  };

  return (
    <SettingsGroup
      title={t("Notifications", "通知")}
      description={t(
        "Desktop and browser notifications when a reply completes, is interrupted, or fails. Applies to TUI and Web only; CLI is unchanged.",
        "答复完成、中断或失败时发送桌面 / 浏览器通知。仅 TUI 与 Web 生效，CLI 不使用。"
      )}
    >
      <div className="settings-form-grid">
        <label className="settings-toggle-field">
          <span>
            <strong>{t("Enable notifications", "启用通知")}</strong>
            <small>{t("Show a system or browser notification after a reply completes, is interrupted, or fails.", "助手答复完成、中断或失败后显示系统或浏览器通知。")}</small>
          </span>
          <input type="checkbox" checked={enabled} onChange={(event) => patch({ enabled: event.target.checked })} />
        </label>
        <label className="settings-toggle-field">
          <span>
            <strong>{t("Play sound", "播放声音")}</strong>
            <small>{t("Play a short cue when a reply completes, is interrupted, or fails.", "答复完成、中断或失败时播放短提示音。")}</small>
          </span>
          <input type="checkbox" checked={sound} onChange={(event) => patch({ sound: event.target.checked })} />
        </label>
      </div>
    </SettingsGroup>
  );
}
