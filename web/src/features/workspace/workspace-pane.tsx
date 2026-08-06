import { useEffect, useState } from "react";
import { ErrorBoundary } from "../../shared/ui/error-boundary/error-boundary";
import { EditorPane } from "./editor-pane";
import { FileTree } from "./file-tree";
import { TerminalDock } from "../terminal/terminal-dock";
import { BackgroundTasksPanel } from "../background-tasks/background-tasks-panel";
import { SubagentWorkspace } from "../subagents/subagent-workspace";
import { SessionSidebar } from "../sessions/session-sidebar";
import type { TerminalManager } from "../terminal/use-terminal-manager";
import { createWorkspacePanelTab, type PaneTab, type WorkspacePanelTab } from "./workspace-tab";
import { workspacePanelTitle } from "./workspace-panel-options";
import { WorkspaceTabBar } from "./workspace-tab-bar";
import "./workspace-pane.css";
import { useI18n } from "../i18n/use-i18n";
import { ensureTerminalTab } from "../terminal/terminal-tab-state";
import { WorkspaceEmptyState } from "./workspace-empty-state";
import type { WorkspacePassiveDiff } from "./workspace-passive-diff";
import { TargetedDiffPane } from "./targeted-diff-pane";
import { WorkspaceFileSplit } from "./workspace-file-split";

type WorkspacePaneProps = {
  selectedFile: string | null;
  activeType: PaneTab | null;
  passiveDiff: WorkspacePassiveDiff | null;
  /** 每次递增都要求编辑器展开文件树。 */
  fileTreeRequestId: number;
  /** 文件树展开请求已经消费。 */
  onFileTreeRequestHandled: () => void;
  maximized: boolean;
  onActiveTypeChange: (tab: PaneTab | null) => void;
  onSelectFile: (path: string) => void;
  onClearFile: () => void;
  onToggleMaximized: () => void;
  onCollapse: () => void;
  terminalManager: TerminalManager;
};

/**
 * 渲染带 Cursor 风格顶部标签栏的右侧工作区。
 *
 * 默认不自动塞一个空编辑器；只有点 `+` 选中、打开文件或外部入口时才建标签。
 *
 * @param props 文件选择、活动类型、布局操作与终端状态
 * @returns 工作区面板
 */
export function WorkspacePane({
  selectedFile,
  activeType,
  passiveDiff,
  fileTreeRequestId,
  onFileTreeRequestHandled,
  maximized,
  onActiveTypeChange,
  onSelectFile,
  onClearFile,
  onToggleMaximized,
  onCollapse,
  terminalManager
}: WorkspacePaneProps) {
  const { locale, t } = useI18n();
  const [fileTreeOpen, setFileTreeOpen] = useState(false);
  const [fileTreeOverlay, setFileTreeOverlay] = useState(false);
  // 初始不预开空编辑器；由 `+` 菜单、打开文件或外部入口创建标签。
  const [tabs, setTabs] = useState<WorkspacePanelTab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);

  useEffect(() => {
    if (fileTreeRequestId <= 0) return;
    setFileTreeOpen(true);
    onFileTreeRequestHandled();
  }, [fileTreeRequestId, onFileTreeRequestHandled]);

  useEffect(() => {
    if (!selectedFile) return;
    setTabs((current) => {
      const existing = current.find((tab) => tab.type === "files" && tab.path === selectedFile);
      if (existing) {
        setActiveTabId(existing.id);
        return current;
      }
      const emptyEditor = current.find((tab) => tab.type === "files" && !tab.path);
      if (emptyEditor) {
        setActiveTabId(emptyEditor.id);
        return current.map((tab) =>
          tab.id === emptyEditor.id
            ? {
                ...tab,
                path: selectedFile,
                title: selectedFile.split("/").filter(Boolean).at(-1) ?? selectedFile,
                closable: true
              }
            : tab
        );
      }
      const created = createWorkspacePanelTab("files", { path: selectedFile }, locale);
      setActiveTabId(created.id);
      return [...current, created];
    });
    onActiveTypeChange("files");
  }, [locale, onActiveTypeChange, selectedFile]);

  // 外部入口或重新打开时：已有则激活，没有则新建。
  useEffect(() => {
    if (!activeType || activeType === "diff") return;
    if (activeType === "terminal") {
      setTabs((current) => {
        const existing = current.find((tab) => tab.type === "terminal" && tab.terminalId === terminalManager.activeId);
        if (existing) {
          setActiveTabId(existing.id);
          return current;
        }
        if (terminalManager.activeId) {
          const terminal = terminalManager.terminals.find((item) => item.id === terminalManager.activeId);
          const created = createWorkspacePanelTab("terminal", {
            title: terminal?.title || t("Terminal", "终端"),
            terminalId: terminalManager.activeId
          }, locale);
          setActiveTabId(created.id);
          return ensureTerminalTab(current, created);
        }
        return current;
      });
      return;
    }
    setTabs((current) => {
      const existing = current.find((tab) => tab.type === activeType);
      if (existing) {
        setActiveTabId((id) => (id === existing.id ? id : existing.id));
        return current;
      }
      const created = createWorkspacePanelTab(activeType, {
        title: panelTitle(activeType, t),
        closable: true
      }, locale);
      setActiveTabId(created.id);
      return [...current, created];
    });
  }, [activeType, locale, terminalManager.activeId, terminalManager.terminals, t]);

  useEffect(() => {
    if (!passiveDiff) return;
    const created = createWorkspacePanelTab("diff", {
      path: passiveDiff.path,
      title: passiveDiff.title,
      diffSource: passiveDiff.source
    }, locale);
    setTabs((current) => {
      const existing = current.find((tab) => tab.id === created.id);
      return existing
        ? current.map((tab) => tab.id === existing.id ? created : tab)
        : [...current, created];
    });
    setActiveTabId(created.id);
    onActiveTypeChange("diff");
  }, [locale, onActiveTypeChange, passiveDiff]);

  useEffect(() => {
    setTabs((current) =>
      current.map((tab) => {
        if (tab.type !== "terminal" || !tab.terminalId) return tab;
        const terminal = terminalManager.terminals.find((item) => item.id === tab.terminalId);
        if (!terminal) return tab;
        const title = terminal.title || t("Terminal", "终端");
        return tab.title === title ? tab : { ...tab, title };
      })
    );
  }, [terminalManager.terminals, t]);

  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? null;

  /**
   * 从文件树打开文件；覆盖式抽屉在选择后自动收起。
   *
   * @param path 工作区内或已获准访问的外部文件路径
   * @returns 无返回值
   */
  const selectFileFromTree = (path: string) => {
    onSelectFile(path);
    if (fileTreeOverlay) setFileTreeOpen(false);
  };

  /**
   * 切换文件树，并在关闭时清理覆盖布局状态。
   *
   * @returns 无返回值
   */
  const toggleFileTree = () => {
    setFileTreeOpen((current) => {
      if (current) setFileTreeOverlay(false);
      return !current;
    });
  };

  const addTab = async (type: PaneTab) => {
    if (type === "files") {
      const created = createWorkspacePanelTab("files", { title: t("Editor", "编辑器") }, locale);
      setTabs((current) => [...current, created]);
      setActiveTabId(created.id);
      onActiveTypeChange("files");
      onClearFile();
      return;
    }
    if (type === "terminal") {
      const terminal = await terminalManager.createTerminal();
      const created = createWorkspacePanelTab("terminal", {
        title: terminal.title || t("Terminal", "终端"),
        terminalId: terminal.id
      }, locale);
      setTabs((current) => ensureTerminalTab(current, created));
      setActiveTabId(created.id);
      onActiveTypeChange("terminal");
      return;
    }
    const existing = tabs.find((tab) => tab.type === type);
    if (existing) {
      setActiveTabId(existing.id);
      onActiveTypeChange(type);
      return;
    }
    const created = createWorkspacePanelTab(type, { title: panelTitle(type, t) }, locale);
    setTabs((current) => [...current, created]);
    setActiveTabId(created.id);
    onActiveTypeChange(type);
  };

  const closeTab = (id: string) => {
    setTabs((current) => {
      const index = current.findIndex((tab) => tab.id === id);
      if (index < 0) return current;
      const closing = current[index];
      if (closing?.type === "terminal" && closing.terminalId) {
        void terminalManager.closeTerminal(closing.terminalId);
      }
      const next = current.filter((tab) => tab.id !== id);
      if (activeTabId === id) {
        const fallback = next[Math.max(0, index - 1)] ?? next[0] ?? null;
        setActiveTabId(fallback?.id ?? null);
        onActiveTypeChange(fallback?.type ?? null);
      }
      if (closing?.type === "files" && closing.path && closing.path === selectedFile) {
        const remainingFile = next.find((tab) => tab.type === "files" && tab.path);
        if (remainingFile?.path) onSelectFile(remainingFile.path);
        else onClearFile();
      }
      return next;
    });
  };

  return (
    <div className="workspace-pane">
      <WorkspaceTabBar
        tabs={tabs}
        activeTabId={activeTab?.id ?? null}
        maximized={maximized}
        onActivate={(id) => {
          setActiveTabId(id);
          const tab = tabs.find((item) => item.id === id);
          if (!tab) return;
          onActiveTypeChange(tab.type);
          if (tab.type === "files" && tab.path) onSelectFile(tab.path);
          if (tab.type === "terminal" && tab.terminalId) terminalManager.setActiveId(tab.terminalId);
        }}
        onClose={closeTab}
        onAdd={(type) => {
          void addTab(type);
        }}
        onToggleMaximized={onToggleMaximized}
        onCollapse={onCollapse}
      />
      <ErrorBoundary key={activeTab?.id ?? "empty"} label={t("This panel failed to render", "该面板渲染失败")}>
      <div className="pane-body">
        {!activeTab && (
          <WorkspaceEmptyState onOpen={(type) => void addTab(type)} />
        )}
        {activeTab?.type === "files" && (
          <WorkspaceFileSplit
            open={fileTreeOpen}
            onOverlayChange={setFileTreeOverlay}
            editor={<EditorPane
              path={activeTab.path ?? selectedFile}
              onSelectFile={onSelectFile}
              fileTreeOpen={fileTreeOpen}
              onToggleFileTree={toggleFileTree}
            />}
            tree={fileTreeOpen ? (
              <FileTree
                selectedFile={activeTab.path ?? selectedFile}
                onSelectFile={selectFileFromTree}
                onClearFile={onClearFile}
                onClose={() => {
                  setFileTreeOpen(false);
                  setFileTreeOverlay(false);
                }}
              />
            ) : null}
          />
        )}
        {activeTab?.type === "diff" && (
          <TargetedDiffPane path={activeTab.path ?? activeTab.title} source={activeTab.diffSource ?? ""} />
        )}
        {activeTab?.type === "terminal" && (
          <TerminalDock terminalId={activeTab.terminalId} title={activeTab.title} error={terminalManager.error} />
        )}
        {activeTab?.type === "tasks" && <BackgroundTasksPanel />}
        {activeTab?.type === "subagents" && <SubagentWorkspace />}
        {activeTab?.type === "sessions" && (
          <div className="workspace-sessions-pane">
            <SessionSidebar collapsed={false} onToggleCollapsed={() => undefined} />
          </div>
        )}
      </div>
      </ErrorBoundary>
    </div>
  );
}

/**
 * 返回当前语言下的工作区面板默认标题。
 *
 * @param type 面板类型
 * @param t 双语文本选择方法
 * @returns 面板标题
 */
function panelTitle(type: PaneTab, t: (en: string, zh: string) => string): string {
  return workspacePanelTitle(type, t);
}
