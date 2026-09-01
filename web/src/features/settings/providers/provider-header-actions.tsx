import { Check, RefreshCw, Trash2 } from "lucide-react";
import type { AppConfig, ProviderConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";

type ProviderHeaderActionsProps = {
  config: AppConfig;
  provider: ProviderConfig;
  enabled: boolean;
  isCurrent: boolean;
  fetching: boolean;
  importBlockedReason: string;
  onToggleEnabled: (enabled: boolean) => void;
  onFetchModels: () => void;
  onSetCurrent: () => void;
  onDelete: () => void;
};

/**
 * 供应商编辑器头部的操作区：启停、导入模型、设为当前、删除。
 *
 * 操作从左到右按破坏性递增排列，删除独占最右；
 * 导入按钮的禁用原因挂在 tooltip 上，包裹层保证禁用态也能看到提示。
 *
 * @param props 配置、供应商状态与操作回调
 * @returns 操作按钮组
 */
export function ProviderHeaderActions({
  config,
  provider,
  enabled,
  isCurrent,
  fetching,
  importBlockedReason,
  onToggleEnabled,
  onFetchModels,
  onSetCurrent,
  onDelete
}: ProviderHeaderActionsProps) {
  const { t } = useI18n();
  const name = provider.display_name || provider.id;

  return (
    <>
      <label className="settings-switch">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) => onToggleEnabled(event.target.checked)}
        />
        <span />
        <strong>{enabled ? t("Enabled", "已启用") : t("Disabled", "已停用")}</strong>
      </label>
      {/* 禁用态按钮收不到鼠标事件，说明挂在包裹层上 */}
      <span className="settings-action-hint" title={importBlockedReason || undefined}>
        <button type="button" className="settings-secondary" onClick={onFetchModels} disabled={fetching || !provider.base_url.trim()}>
          <RefreshCw size={14} className={fetching ? "spin" : ""} />
          {fetching ? t("Fetching", "正在获取") : t("Import models", "导入模型")}
        </button>
      </span>
      <button
        type="button"
        className={isCurrent ? "settings-secondary active" : "settings-secondary"}
        onClick={onSetCurrent}
        disabled={isCurrent || !enabled}
        title={isCurrent
          ? t(`“${name}” is the provider new sessions use.`, `“${name}”是新会话使用的供应商。`)
          : t("Make this the provider new sessions use", "将新会话的供应商切换为它")}
      >
        <Check size={14} />
        {isCurrent ? t("Current provider", "当前供应商") : t("Set as current", "设为当前")}
      </button>
      <button type="button" className="settings-danger" onClick={onDelete}>
        <Trash2 size={14} />
        {t("Delete provider", "删除供应商")}
      </button>
      {config.providers.length > 1 && (
        <span className="provider-header-meta">
          {t(
            `${config.providers.filter((item) => item.enabled !== false).length} of ${config.providers.length} enabled`,
            `${config.providers.filter((item) => item.enabled !== false).length}/${config.providers.length} 启用`
          )}
        </span>
      )}
    </>
  );
}
