import { useI18n } from "../i18n/use-i18n";
import { ACTIVE_WORKSPACE_PANEL_OPTIONS, type WorkspacePanelAction } from "./workspace-panel-options";

type WorkspaceEmptyStateProps = {
  onOpen: (type: WorkspacePanelAction) => void;
};

const PANEL_GROUPS: { titleEn: string; titleZh: string; types: WorkspacePanelAction[] }[] = [
  { titleEn: "Edit", titleZh: "编辑", types: ["files", "diff"] },
  { titleEn: "Terminal", titleZh: "终端", types: ["terminal", "ssh"] },
  { titleEn: "Run", titleZh: "运行", types: ["tasks", "subagents", "side-chat"] }
];

/**
 * 渲染刚打开但尚未选择功能的右侧侧栏。
 *
 * 参数:
 * - `props`: 功能打开回调
 *
 * 返回:
 * - 按用途分组的侧栏功能引导页
 */
export function WorkspaceEmptyState({ onOpen }: WorkspaceEmptyStateProps) {
  const { t } = useI18n();
  const options = new Map(ACTIVE_WORKSPACE_PANEL_OPTIONS.map((option) => [option.type, option]));
  return (
    <div className="workspace-pane-empty">
      <div className="workspace-pane-empty-heading">
        <strong>{t("Open a view", "打开一个视图")}</strong>
        <span>{t("It stays in this side panel", "内容会留在这个侧栏里")}</span>
      </div>
      {PANEL_GROUPS.map((group) => (
        <section key={group.titleEn} className="workspace-pane-empty-group">
          <h3>{t(group.titleEn, group.titleZh)}</h3>
          <div className="workspace-pane-empty-actions">
            {group.types.map((type) => {
              const option = options.get(type);
              if (!option) return null;
              const Icon = option.icon;
              return (
                <button type="button" key={option.type} onClick={() => onOpen(option.type)}>
                  <Icon size={15} aria-hidden />
                  <span>{t(option.labelEn, option.labelZh)}</span>
                </button>
              );
            })}
          </div>
        </section>
      ))}
    </div>
  );
}
