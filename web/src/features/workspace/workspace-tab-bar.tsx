import { Activity, Bot, FileCode2, GitCompareArrows, Maximize2, Minimize2, PanelRightClose, Plus, SquareTerminal, X } from "lucide-react";
import { useRef, useState } from "react";
import { useOutsidePointerDown } from "../../shared/hooks/use-outside-pointer-down";
import type { PaneTab, WorkspacePanelTab } from "./workspace-tab";
import { useI18n } from "../i18n/use-i18n";

type WorkspaceTabBarProps = {
  tabs: WorkspacePanelTab[];
  activeTabId: string | null;
  maximized: boolean;
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
  onAdd: (type: PaneTab) => void;
  onToggleMaximized: () => void;
  onCollapse: () => void;
};

const addable: Array<{ type: PaneTab; labelEn: string; labelZh: string; icon: typeof FileCode2 }> = [
  { type: "files", labelEn: "Editor", labelZh: "编辑器", icon: FileCode2 },
  { type: "diff", labelEn: "Git", labelZh: "Git", icon: GitCompareArrows },
  { type: "terminal", labelEn: "Terminal", labelZh: "终端", icon: SquareTerminal },
  { type: "tasks", labelEn: "Background tasks", labelZh: "后台任务", icon: Activity },
  { type: "subagents", labelEn: "Subagents", labelZh: "子智能体", icon: Bot }
];

/**
 * 渲染 Cursor 风格的工作区顶部标签栏。
 *
 * 标签可横向滚动；`+` 贴在末标签右侧（标签少时紧贴末标签，多时贴在滚动区末尾），
 * 全屏/收起始终固定在栏右侧。下拉菜单挂在滚动区外，避免被 overflow 裁切。
 *
 * @param props 标签列表、当前标签与布局操作
 * @returns 工作区标签导航
 */
export function WorkspaceTabBar(props: WorkspaceTabBarProps) {
  const { t } = useI18n();
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  useOutsidePointerDown(menuRef, () => setMenuOpen(false), menuOpen);

  return (
    <div className="workspace-tab-bar" role="tablist" aria-label={t("Workspace tabs", "工作区标签")}>
      <div className="workspace-tab-scroll-row">
        <div className="workspace-tab-scroll">
          {props.tabs.map((tab) => {
            const active = tab.id === props.activeTabId;
            return (
              <div key={tab.id} className={active ? "workspace-tab active" : "workspace-tab"} role="presentation">
                <button
                  type="button"
                  role="tab"
                  aria-selected={active}
                  className="workspace-tab-main"
                  onClick={() => props.onActivate(tab.id)}
                  title={tab.path ?? tab.title}
                >
                  <TabIcon type={tab.type} />
                  <span>{tab.title || translatedPaneLabel(tab.type, t)}</span>
                </button>
                {tab.closable && (
                  <button
                    type="button"
                    className="workspace-tab-close"
                    aria-label={t(`Close ${tab.title}`, `关闭 ${tab.title}`)}
                    onClick={(event) => {
                      event.stopPropagation();
                      props.onClose(tab.id);
                    }}
                  >
                    <X size={12} />
                  </button>
                )}
              </div>
            );
          })}
        </div>
        <div className="workspace-tab-actions" ref={menuRef}>
          <button
            type="button"
            className="workspace-tab-add"
            aria-label={t("Add panel", "添加面板")}
            title={t("Add panel", "添加面板")}
            aria-expanded={menuOpen}
            onClick={() => setMenuOpen((value) => !value)}
          >
            <Plus size={14} />
          </button>
          {menuOpen && (
            <div className="workspace-tab-add-menu" role="menu">
              {addable.map((item) => {
                const Icon = item.icon;
                return (
                  <button
                    type="button"
                    role="menuitem"
                    key={item.type}
                    onClick={() => {
                      props.onAdd(item.type);
                      setMenuOpen(false);
                    }}
                  >
                    <Icon size={14} />
                    <span>{t(item.labelEn, item.labelZh)}</span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </div>
      <div className="workspace-tab-layout">
        <button
          type="button"
          className={props.maximized ? "active" : ""}
          onClick={props.onToggleMaximized}
          title={props.maximized ? t("Exit full screen", "退出全屏") : t("Full screen", "全屏")}
          aria-label={props.maximized ? t("Exit full screen", "退出全屏") : t("Full screen", "全屏")}
          aria-pressed={props.maximized}
        >
          {props.maximized ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
        </button>
        <button type="button" onClick={props.onCollapse} title={t("Collapse workspace", "收起工作区")} aria-label={t("Collapse workspace", "收起工作区")}>
          <PanelRightClose size={14} />
        </button>
      </div>
    </div>
  );
}

/**
 * 返回当前语言下的面板标题。
 *
 * @param type 面板类型
 * @param t 双语文本选择方法
 * @returns 面板标题
 */
function translatedPaneLabel(type: PaneTab, t: (en: string, zh: string) => string): string {
  return {
    files: t("Editor", "编辑器"),
    diff: "Git",
    terminal: t("Terminal", "终端"),
    tasks: t("Background tasks", "后台任务"),
    subagents: t("Subagents", "子智能体")
  }[type];
}

function TabIcon({ type }: { type: PaneTab }) {
  if (type === "diff") return <GitCompareArrows size={13} />;
  if (type === "terminal") return <SquareTerminal size={13} />;
  if (type === "tasks") return <Activity size={13} />;
  if (type === "subagents") return <Bot size={13} />;
  return <FileCode2 size={13} />;
}
