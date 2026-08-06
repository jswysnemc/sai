import { ChevronRight, PanelRightOpen } from "lucide-react";
import { useI18n } from "../i18n/use-i18n";
import { ACTIVE_WORKSPACE_PANEL_OPTIONS } from "./workspace-panel-options";
import type { PaneTab } from "./workspace-tab";

type WorkspaceEmptyStateProps = {
  onOpen: (type: PaneTab) => void;
};

/**
 * 渲染刚打开但尚未选择功能的右侧侧栏。
 *
 * 参数:
 * - `props`: 功能打开回调
 *
 * 返回:
 * - 紧凑的侧栏功能引导页
 */
export function WorkspaceEmptyState({ onOpen }: WorkspaceEmptyStateProps) {
  const { t } = useI18n();
  return (
    <div className="workspace-pane-empty">
      <div className="workspace-pane-empty-heading">
        <PanelRightOpen size={18} aria-hidden />
        <div>
          <strong>{t("Open a workspace view", "打开侧栏功能")}</strong>
          <span>{t("Choose a view for this side panel", "选择要在此侧栏显示的功能")}</span>
        </div>
      </div>
      <div className="workspace-pane-empty-actions">
        {ACTIVE_WORKSPACE_PANEL_OPTIONS.map((option) => {
          const Icon = option.icon;
          return (
            <button type="button" key={option.type} onClick={() => onOpen(option.type)}>
              <Icon size={15} aria-hidden />
              <span>{t(option.labelEn, option.labelZh)}</span>
              <ChevronRight size={13} aria-hidden />
            </button>
          );
        })}
      </div>
    </div>
  );
}
