import { useEffect, useState } from "react";
import { DiffPane } from "./diff-pane";
import { EditorPane } from "./editor-pane";
import { FileTree } from "./file-tree";
import { TerminalDock } from "../terminal/terminal-dock";
import { BackgroundTasksPanel } from "../background-tasks/background-tasks-panel";
import { SubagentWorkspace } from "../subagents/subagent-workspace";
import type { TerminalManager } from "../terminal/use-terminal-manager";
import { createWorkspacePanelTab, type PaneTab, type WorkspacePanelTab } from "./workspace-tab";
import { WorkspaceTabBar } from "./workspace-tab-bar";
import "./workspace-pane.css";

type WorkspacePaneProps = {
  selectedFile: string | null;
  activeType: PaneTab;
  maximized: boolean;
  onActiveTypeChange: (tab: PaneTab) => void;
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
  maximized,
  onActiveTypeChange,
  onSelectFile,
  onClearFile,
  onToggleMaximized,
  onCollapse,
  terminalManager
}: WorkspacePaneProps) {
  const [fileTreeOpen, setFileTreeOpen] = useState(false);
  // 初始不预开空编辑器；由 `+` 菜单、打开文件或外部入口创建标签。
  const [tabs, setTabs] = useState<WorkspacePanelTab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);

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
      const created = createWorkspacePanelTab("files", { path: selectedFile });
      setActiveTabId(created.id);
      return [...current, created];
    });
    onActiveTypeChange("files");
  }, [onActiveTypeChange, selectedFile]);

  // 外部入口或重新打开时：已有则激活，没有则新建。
  useEffect(() => {
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
            title: terminal?.title || "终端",
            terminalId: terminalManager.activeId
          });
          setActiveTabId(created.id);
          return [...current, created];
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
        title: activeType === "files" ? "编辑器" : undefined,
        closable: true
      });
      setActiveTabId(created.id);
      return [...current, created];
    });
  }, [activeType, terminalManager.activeId, terminalManager.terminals]);

  useEffect(() => {
    setTabs((current) =>
      current.map((tab) => {
        if (tab.type !== "terminal" || !tab.terminalId) return tab;
        const terminal = terminalManager.terminals.find((item) => item.id === tab.terminalId);
        if (!terminal) return tab;
        const title = terminal.title || "终端";
        return tab.title === title ? tab : { ...tab, title };
      })
    );
  }, [terminalManager.terminals]);

  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? null;

  const addTab = async (type: PaneTab) => {
    if (type === "files") {
      const created = createWorkspacePanelTab("files", { title: "编辑器" });
      setTabs((current) => [...current, created]);
      setActiveTabId(created.id);
      onActiveTypeChange("files");
      onClearFile();
      return;
    }
    if (type === "terminal") {
      const terminal = await terminalManager.createTerminal();
      const created = createWorkspacePanelTab("terminal", {
        title: terminal.title || "终端",
        terminalId: terminal.id
      });
      setTabs((current) => [...current, created]);
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
    const created = createWorkspacePanelTab(type);
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
        if (fallback) onActiveTypeChange(fallback.type);
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
      <div className="pane-body">
        {!activeTab && (
          <div className="workspace-pane-empty">
            <p>没有打开的面板</p>
            <span>点上方 + 选择要打开的组件</span>
          </div>
        )}
        {activeTab?.type === "files" && (
          <div className={fileTreeOpen ? "files-layout file-tree-open" : "files-layout file-tree-closed"}>
            <EditorPane
              path={activeTab.path ?? selectedFile}
              onSelectFile={onSelectFile}
              fileTreeOpen={fileTreeOpen}
              onToggleFileTree={() => setFileTreeOpen((value) => !value)}
            />
            {fileTreeOpen && (
              <FileTree
                selectedFile={activeTab.path ?? selectedFile}
                onSelectFile={onSelectFile}
                onClearFile={onClearFile}
                onClose={() => setFileTreeOpen(false)}
              />
            )}
          </div>
        )}
        {activeTab?.type === "diff" && <DiffPane />}
        {activeTab?.type === "terminal" && (
          <TerminalDock terminalId={activeTab.terminalId} error={terminalManager.error} />
        )}
        {activeTab?.type === "tasks" && <BackgroundTasksPanel />}
        {activeTab?.type === "subagents" && <SubagentWorkspace />}
      </div>
    </div>
  );
}
