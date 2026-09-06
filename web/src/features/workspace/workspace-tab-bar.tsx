import { Activity, Bot, Check, ChevronDown, ChevronLeft, FileCode2, GitCompareArrows, Maximize2, MessageSquarePlus, Minimize2, PanelRightClose, Plus, SquareTerminal, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { Button } from "../../shared/ui/button/button";
import { FileTypeIcon } from "../../shared/ui/file-icon";
import { ActionMenu } from "../../shared/ui/menu/action-menu";
import type { PaneTab, WorkspacePanelTab } from "./workspace-tab";
import { ACTIVE_WORKSPACE_PANEL_OPTIONS, workspacePanelTitle, type WorkspacePanelAction } from "./workspace-panel-options";
import { fileTreeGitStatusLabel, fileTreeGitStatusTone } from "./use-file-tree-git";
import type { FileTreeGitEntry } from "./use-workspace-git-entries";
import { useI18n } from "../i18n/use-i18n";
import { workspaceTabDirectoryLabels } from "./workspace-tab-labels";

type WorkspaceTabBarProps = {
  tabs: WorkspacePanelTab[];
  activeTabId: string | null;
  maximized: boolean;
  gitEntries?: ReadonlyMap<string, FileTreeGitEntry>;
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
  onAdd: (type: WorkspacePanelAction) => void;
  onToggleMaximized: () => void;
  onCollapse: () => void;
};

/**
 * 渲染工作区标签、面板菜单与布局操作，支持方向键和 Delete 关闭。
 * @param props 标签、Git 状态与导航回调
 * @returns 支持横向滚动和键盘操作的标签栏
 */
export function WorkspaceTabBar(props: WorkspaceTabBarProps) {
  const { t } = useI18n();
  const tabsRef = useRef<HTMLDivElement>(null);
  const [overflowing, setOverflowing] = useState(false);
  const directories = useMemo(() => workspaceTabDirectoryLabels(props.tabs), [props.tabs]);

  useEffect(() => {
    const scroller = tabsRef.current;
    if (!scroller) return;
    /**
     * 检查标签溢出，并将当前标签保持在可视范围内。
     * @returns 无返回值
     */
    const updateOverflow = () => {
      setOverflowing(scroller.scrollWidth > scroller.clientWidth + 1);
      scroller.querySelector<HTMLElement>('[aria-selected="true"]')
        ?.scrollIntoView({ block: "nearest", inline: "nearest" });
    };
    updateOverflow();
    const observer = new ResizeObserver(updateOverflow);
    observer.observe(scroller);
    return () => observer.disconnect();
  }, [props.activeTabId, props.tabs]);

  /**
   * 切换键盘焦点并同步激活标签。
   * @param event 标签列表键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!(event.target instanceof HTMLElement) || event.target.getAttribute("role") !== "tab") return;
    const index = props.tabs.findIndex((tab) => tab.id === props.activeTabId);
    if (event.key === "Delete" && props.tabs[index]?.closable) {
      event.preventDefault();
      props.onClose(props.tabs[index].id);
      requestAnimationFrame(() => tabsRef.current?.querySelector<HTMLButtonElement>('[role="tab"][aria-selected="true"]')?.focus());
      return;
    }
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key) || !props.tabs.length) return;
    event.preventDefault();
    const next = event.key === "Home" ? 0 : event.key === "End" ? props.tabs.length - 1
      : (index + (event.key === "ArrowRight" ? 1 : -1) + props.tabs.length) % props.tabs.length;
    props.onActivate(props.tabs[next].id);
    const button = tabsRef.current?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next];
    button?.focus();
    button?.scrollIntoView({ block: "nearest", inline: "nearest" });
  };

  return (
    <div className="workspace-tab-bar">
      <Button variant="ghost" className="workspace-tab-back md:hidden" onClick={props.onCollapse} aria-label={t("Back to chat", "返回聊天")}>
        <ChevronLeft size={16} /><span>{t("Chat", "聊天")}</span>
      </Button>
      <div className="workspace-tab-scroll-row">
        <div ref={tabsRef} className="workspace-tab-scroll" role="tablist" aria-label={t("Workspace tabs", "工作区标签")} onKeyDown={handleKeyDown}>
          {props.tabs.map((tab) => {
            const active = tab.id === props.activeTabId;
            const gitEntry = tab.type === "files" && tab.path ? props.gitEntries?.get(tab.path) : undefined;
            const title = tab.title || workspacePanelTitle(tab.type, t);
            const directory = directories.get(tab.id);
            return (
              <div key={tab.id} className={active ? "workspace-tab active" : "workspace-tab"} role="presentation">
                <Button
                  variant="ghost"
                  role="tab"
                  id={`workspace-tab-${tab.id}`}
                  aria-controls="workspace-panel-content"
                  aria-selected={active}
                  tabIndex={active ? 0 : -1}
                  className="workspace-tab-main"
                  onClick={() => props.onActivate(tab.id)}
                  title={tab.path ?? title}
                >
                  <TabIcon type={tab.type} path={tab.path} />
                  <span className="workspace-tab-label">
                    <span className="workspace-tab-title">{title}</span>
                    {directory && <small className="workspace-tab-directory">{directory}</small>}
                  </span>
                  {gitEntry && <em className={`workspace-tab-git git-${fileTreeGitStatusTone(gitEntry.entry)}`} aria-hidden>{fileTreeGitStatusLabel(gitEntry.entry)}</em>}
                </Button>
                {tab.closable && (
                  <Button variant="ghost" size="icon" className="workspace-tab-close" tabIndex={active ? 0 : -1} aria-label={t(`Close ${title}`, `关闭 ${title}`)} onClick={() => props.onClose(tab.id)}>
                    <X size={12} />
                  </Button>
                )}
              </div>
            );
          })}
        </div>
        {overflowing && (
          <ActionMenu
            className="workspace-tab-actions"
            label={t(`Open tabs (${props.tabs.length})`, `已打开的标签（${props.tabs.length}）`)}
            trigger={<ChevronDown size={15} />}
            items={props.tabs.map((tab) => ({
              id: tab.id,
              label: tab.path ?? (tab.title || workspacePanelTitle(tab.type, t)),
              icon: tab.id === props.activeTabId ? <Check size={14} /> : <TabIcon type={tab.type} path={tab.path} />,
              onSelect: () => props.onActivate(tab.id),
            }))}
          />
        )}
        <ActionMenu
          className="workspace-tab-actions"
          label={t("Add panel", "添加面板")}
          trigger={<Plus size={15} />}
          items={ACTIVE_WORKSPACE_PANEL_OPTIONS.map((item) => ({
            id: item.type,
            label: t(item.labelEn, item.labelZh),
            icon: <item.icon size={15} />,
            onSelect: () => props.onAdd(item.type)
          }))}
        />
      </div>
      <div className="workspace-tab-layout hidden md:flex">
        <Button variant="ghost" size="icon" onClick={props.onToggleMaximized} title={props.maximized ? t("Exit full screen", "退出全屏") : t("Full screen", "全屏")} aria-label={props.maximized ? t("Exit full screen", "退出全屏") : t("Full screen", "全屏")} aria-pressed={props.maximized}>
          {props.maximized ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
        </Button>
        <Button variant="ghost" size="icon" onClick={props.onCollapse} title={t("Collapse workspace", "收起工作区")} aria-label={t("Collapse workspace", "收起工作区")}><PanelRightClose size={14} /></Button>
      </div>
    </div>
  );
}

/**
 * 为每种面板提供同一图标体系中的标识。
 * @param props 面板类型
 * @returns 对应面板图标
 */
function TabIcon({ type, path }: { type: PaneTab; path?: string }) {
  if (type === "diff") return <GitCompareArrows size={14} aria-hidden />;
  if (type === "terminal") return <SquareTerminal size={14} aria-hidden />;
  if (type === "tasks") return <Activity size={14} aria-hidden />;
  if (type === "subagents") return <Bot size={14} aria-hidden />;
  if (type === "side-chat") return <MessageSquarePlus size={14} aria-hidden />;
  if (path) return <FileTypeIcon name={path} size={14} />;
  return <FileCode2 size={14} aria-hidden />;
}
