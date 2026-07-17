import { ArrowLeftRight, Activity, Bot, ChevronsLeft, ChevronsRight, FileCode2, GitCompareArrows, LayoutPanelLeft, Maximize2, MessageSquare, SlidersHorizontal, SquareTerminal } from "lucide-react";
import { useRef, useState } from "react";
import { useOutsidePointerDown } from "../../shared/hooks/use-outside-pointer-down";
import type { PaneTab } from "./workspace-tab";
import { useI18n } from "../i18n/use-i18n";

type WorkspaceActivityRailProps = {
  tab: PaneTab;
  workspaceOpen: boolean;
  chatOpen: boolean;
  maximized: boolean;
  onSelectTab: (tab: PaneTab) => void;
  onCollapse: () => void;
  onExpand: () => void;
  onToggleChat: () => void;
  onToggleMaximized: () => void;
  onToggleSwapped: () => void;
};

/**
 * 渲染贴右侧边缘的 Cursor 风格活动栏。
 *
 * 上段切换工作区视图；下段提供布局菜单与展开/收起。
 *
 * @param props 当前视图、布局状态与各切换回调
 * @returns 右侧活动栏
 */
export function WorkspaceActivityRail(props: WorkspaceActivityRailProps) {
  const { t } = useI18n();
  const tabs: Array<{ id: PaneTab; label: string; icon: typeof FileCode2 }> = [
    { id: "files", label: t("Files", "文件"), icon: FileCode2 },
    { id: "diff", label: "Git", icon: GitCompareArrows },
    { id: "terminal", label: t("Terminal", "终端"), icon: SquareTerminal },
    { id: "tasks", label: t("Background tasks", "后台任务"), icon: Activity },
    { id: "subagents", label: t("Subagents", "子智能体"), icon: Bot }
  ];
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  useOutsidePointerDown(menuRef, () => setMenuOpen(false), menuOpen);

  /**
   * 处理视图图标点击:未打开则打开并切换,已打开的当前视图再点则收起。
   *
   * @param id 目标视图
   */
  const handleTab = (id: PaneTab) => {
    if (props.workspaceOpen && props.tab === id) {
      props.onCollapse();
      return;
    }
    props.onSelectTab(id);
  };

  return (
    <nav className="workspace-activity-rail" aria-label={t("Workspace activity bar", "工作区活动栏")}>
      <div className="workspace-activity-rail-top">
        {tabs.map(({ id, label, icon: Icon }) => {
          const active = props.workspaceOpen && props.tab === id;
          return (
            <button key={id} type="button" className={active ? "active" : ""} onClick={() => handleTab(id)} title={active ? t(`Collapse ${label}`, `收起${label}`) : label} aria-label={label} aria-pressed={active}>
              <Icon size={16} />
            </button>
          );
        })}
      </div>
      <div className="workspace-activity-rail-bottom">
        <div className="rail-menu-anchor" ref={menuRef}>
          <button type="button" className={menuOpen ? "active" : ""} onClick={() => setMenuOpen((value) => !value)} title={t("Layout options", "布局选项")} aria-label={t("Layout options", "布局选项")} aria-expanded={menuOpen}>
            <SlidersHorizontal size={15} />
          </button>
          {menuOpen && (
            <div className="rail-layout-menu" role="menu">
              <button type="button" role="menuitem" className={props.chatOpen ? "checked" : ""} onClick={props.onToggleChat}>
                <MessageSquare size={14} /><span>{t("Chat area", "聊天区")}</span>
              </button>
              <button type="button" role="menuitem" className={props.maximized ? "checked" : ""} onClick={props.onToggleMaximized}>
                <Maximize2 size={14} /><span>{t("Maximize workspace", "工作区全屏")}</span>
              </button>
              <button type="button" role="menuitem" onClick={props.onToggleSwapped}>
                <ArrowLeftRight size={14} /><span>{t("Swap left and right layout", "交换左右布局")}</span>
              </button>
            </div>
          )}
        </div>
        <button
          type="button"
          onClick={props.workspaceOpen ? props.onCollapse : props.onExpand}
          title={props.workspaceOpen ? t("Collapse workspace", "收起工作区") : t("Expand workspace", "展开工作区")}
          aria-label={props.workspaceOpen ? t("Collapse workspace", "收起工作区") : t("Expand workspace", "展开工作区")}
        >
          {props.workspaceOpen ? <ChevronsRight size={16} /> : <ChevronsLeft size={16} />}
        </button>
        {!props.chatOpen && !props.workspaceOpen && (
          <button type="button" onClick={props.onToggleChat} title={t("Show chat area", "显示聊天区")} aria-label={t("Show chat area", "显示聊天区")}>
            <LayoutPanelLeft size={16} />
          </button>
        )}
      </div>
    </nav>
  );
}
