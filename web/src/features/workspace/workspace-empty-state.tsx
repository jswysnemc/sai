import { useI18n } from "../i18n/use-i18n";
import { ACTIVE_WORKSPACE_PANEL_OPTIONS, type WorkspacePanelAction } from "./workspace-panel-options";
import { modKeyLabel } from "../../shared/mod-key";

type WorkspaceEmptyStateProps = {
  onOpen: (type: WorkspacePanelAction) => void;
};

/** 空侧栏里需要露出的快捷键，没有登记的功能只显示名称。 */
const PANEL_SHORTCUTS: Partial<Record<WorkspacePanelAction, string>> = {
  diff: "Shift+G",
  terminal: "J",
  files: "Shift+E"
};

/**
 * 渲染刚打开但尚未选择功能的右侧侧栏。
 *
 * 参数:
 * - `props`: 功能打开回调
 *
 * 返回:
 * - 居中的功能列表
 */
export function WorkspaceEmptyState({ onOpen }: WorkspaceEmptyStateProps) {
  const { t } = useI18n();
  const modifier = modKeyLabel();
  return (
    <div className="workspace-pane-empty">
      <div className="workspace-pane-empty-actions">
        {ACTIVE_WORKSPACE_PANEL_OPTIONS.map((option) => {
          const Icon = option.icon;
          const shortcut = PANEL_SHORTCUTS[option.type];
          return (
            <button type="button" key={option.type} onClick={() => onOpen(option.type)}>
              <Icon size={16} aria-hidden />
              <span>{t(option.labelEn, option.labelZh)}</span>
              {shortcut ? <kbd>{modifier}+{shortcut}</kbd> : <kbd aria-hidden="true" />}
            </button>
          );
        })}
      </div>
    </div>
  );
}
