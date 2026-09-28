import { Check, RefreshCw } from "../../../shared/ui/icons";
import type { ProviderConfig } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { InlineSwitch } from "../kit";
import { useI18n } from "../../i18n/use-i18n";

type ProviderHeaderActionsProps = {
  provider: ProviderConfig;
  enabled: boolean;
  isCurrent: boolean;
  fetching: boolean;
  importBlockedReason: string;
  onToggleEnabled: (enabled: boolean) => void;
  onFetchModels: () => void;
  onSetCurrent: () => void;
};

/**
 * 供应商编辑器头部的常用操作区：启停、导入模型与设为当前。
 *
 * 删除由详情标题的更多菜单承载；导入按钮在禁用时仍可查看原因。
 *
 * @param props 配置、供应商状态与操作回调
 * @returns 操作按钮组
 */
export function ProviderHeaderActions({
  provider,
  enabled,
  isCurrent,
  fetching,
  importBlockedReason,
  onToggleEnabled,
  onFetchModels,
  onSetCurrent
}: ProviderHeaderActionsProps) {
  const { t } = useI18n();
  const name = provider.display_name || provider.id;

  return (
    <>
      <InlineSwitch checked={enabled} onChange={onToggleEnabled} label={enabled ? t("Enabled", "已启用") : t("Disabled", "已停用")} />
      {/* 禁用态按钮收不到鼠标事件，说明挂在包裹层上 */}
      <span className="settings-action-hint" title={importBlockedReason || undefined}>
        <Button size="small" onClick={onFetchModels} disabled={fetching || !provider.base_url.trim()}>
          <RefreshCw size={14} className={fetching ? "spin" : ""} />
          {fetching ? t("Fetching", "正在获取") : t("Import models", "导入模型")}
        </Button>
      </span>
      <Button
        size="small"
        className={isCurrent ? "settings-secondary active" : "settings-secondary"}
        onClick={onSetCurrent}
        disabled={isCurrent || !enabled}
        title={isCurrent
          ? t(`“${name}” is the provider new sessions use.`, `“${name}”是新会话使用的供应商。`)
          : t("Make this the provider new sessions use", "将新会话的供应商切换为它")}
      >
        <Check size={14} />
        {isCurrent ? t("Current provider", "当前供应商") : t("Set as current", "设为当前")}
      </Button>
    </>
  );
}
