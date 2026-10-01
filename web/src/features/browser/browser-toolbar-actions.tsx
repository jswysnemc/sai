import { Bug, Ellipsis, ExternalLink, MonitorSmartphone, MousePointerClick, Trash2 } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { ActionMenu, type ActionMenuItem } from "../../shared/ui/menu/action-menu";
import { useI18n } from "../i18n/use-i18n";

type BrowserToolbarActionsProps = {
  disabled: boolean;
  responsive: boolean;
  picking: boolean;
  currentUrl: string;
  /** 本机访问时可用的调试工具地址 */
  devtoolsUrl: string | null;
  onToggleResponsive: () => void;
  onTogglePicking: () => void;
  onClearData: () => void;
};

/**
 * 地址栏右侧的三个操作：自由尺寸、选择网页元素加入聊天、更多操作菜单。
 *
 * @param props 当前状态与操作回调
 * @returns 工具栏操作按钮组
 */
export function BrowserToolbarActions({
  disabled,
  responsive,
  picking,
  currentUrl,
  devtoolsUrl,
  onToggleResponsive,
  onTogglePicking,
  onClearData
}: BrowserToolbarActionsProps) {
  const { t } = useI18n();
  const webPage = /^https?:\/\//i.test(currentUrl);
  const items: ActionMenuItem[] = [
    {
      id: "open-external",
      label: t("Open in default browser", "在默认浏览器中打开"),
      icon: <ExternalLink size={14} aria-hidden />,
      disabled: !webPage,
      onSelect: () => window.open(currentUrl, "_blank", "noopener,noreferrer")
    },
    {
      id: "devtools",
      label: devtoolsUrl ? t("Open DevTools", "打开调试工具") : t("DevTools (local access only)", "调试工具（仅本机访问可用）"),
      icon: <Bug size={14} aria-hidden />,
      disabled: !devtoolsUrl,
      onSelect: () => devtoolsUrl && window.open(devtoolsUrl, "_blank", "noopener,noreferrer")
    },
    {
      id: "clear-data",
      label: t("Clear browsing data", "清除浏览数据"),
      icon: <Trash2 size={14} aria-hidden />,
      danger: true,
      separator: true,
      onSelect: onClearData
    }
  ];
  return (
    <div className="browser-toolbar-actions">
      <Button
        variant="ghost"
        size="icon"
        className={responsive ? "browser-toggle-on" : ""}
        aria-pressed={responsive}
        aria-label={responsive ? t("Exit free size", "退出自由尺寸") : t("Free size", "自由尺寸")}
        title={responsive ? t("Exit free size", "退出自由尺寸") : t("Free size", "自由尺寸")}
        onClick={onToggleResponsive}
      >
        <MonitorSmartphone size={14} aria-hidden />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className={picking ? "browser-toggle-on" : ""}
        aria-pressed={picking}
        aria-label={picking ? t("Cancel element selection", "取消网页元素选择") : t("Select an element to add to chat", "选择网页元素加入聊天")}
        title={picking ? t("Cancel element selection", "取消网页元素选择") : t("Select an element to add to chat", "选择网页元素加入聊天")}
        disabled={disabled}
        onClick={onTogglePicking}
      >
        <MousePointerClick size={14} aria-hidden />
      </Button>
      <ActionMenu
        label={t("More browser actions", "更多浏览器操作")}
        trigger={<Ellipsis size={14} aria-hidden />}
        triggerClassName="ui-button ghost ui-button-icon"
        items={items}
      />
    </div>
  );
}
