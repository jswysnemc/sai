import { lazy, Suspense, useEffect, useState } from "react";
import { ErrorBoundary } from "../../shared/ui/error-boundary/error-boundary";
import { LoadingPanel } from "../../shared/ui/loading-panel";
import { FileTree } from "./file-tree";
import type { TerminalManager } from "../terminal/use-terminal-manager";
import { createWorkspacePanelTab, type PaneTab, type WorkspacePanelTab } from "./workspace-tab";
import { workspacePanelTitle, type WorkspacePanelAction } from "./workspace-panel-options";
import { WorkspaceTabBar } from "./workspace-tab-bar";
import "./workspace-pane.css";
import { useI18n } from "../i18n/use-i18n";
import { ensureTerminalTab } from "../terminal/terminal-tab-state";
import { WorkspaceEmptyState } from "./workspace-empty-state";
import { SshHostPickerDialog } from "../terminal/ssh-host-picker-dialog";
import type { WorkspacePassiveDiff } from "./workspace-passive-diff";
import { WorkspaceFileSplit } from "./workspace-file-split";
import type { SideConversationRequest } from "../side-conversation/side-conversation-events";
import { useWorkspaceGitEntries } from "./use-workspace-git-entries";
import { useFileNavigationHistory } from "./use-file-navigation-history";
import { selectExistingWorkspacePanel } from "./workspace-panel-selection";

// 1. 【前端性能】【面板加载】空工作区只加载导航，各功能在创建面板后加载
const EditorPane = lazy(() => import("./editor-pane").then((module) => ({ default: module.EditorPane })));
const TerminalDock = lazy(() => import("../terminal/terminal-dock").then((module) => ({ default: module.TerminalDock })));
const BackgroundTasksPanel = lazy(() => import("../background-tasks/background-tasks-panel").then((module) => ({ default: module.BackgroundTasksPanel })));
const SubagentWorkspace = lazy(() => import("../subagents/subagent-workspace").then((module) => ({ default: module.SubagentWorkspace })));
const TargetedDiffPane = lazy(() => import("./targeted-diff-pane").then((module) => ({ default: module.TargetedDiffPane })));
const SourceControlPane = lazy(() => import("../source-control/source-control-pane").then((module) => ({ default: module.SourceControlPane })));
const SideConversationPane = lazy(() => import("../side-conversation/side-conversation-pane").then((module) => ({ default: module.SideConversationPane })));

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
  sideConversationRequest: SideConversationRequest | null;
  onRequestSideConversation: () => void;
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
  terminalManager,
  sideConversationRequest,
  onRequestSideConversation
}: WorkspacePaneProps) {
  const { locale, t } = useI18n();
  const [fileTreeOpen, setFileTreeOpen] = useState(false);
  const [sshPickerOpen, setSshPickerOpen] = useState(false);
  const [fileTreeOverlay, setFileTreeOverlay] = useState(false);
  // 初始不预开空编辑器；由 `+` 菜单、打开文件或外部入口创建标签。
  const [tabs, setTabs] = useState<WorkspacePanelTab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  // 标签徽标、编辑器行装饰共享同一份 Git 状态查询
  const { entries: gitEntries } = useWorkspaceGitEntries();
  const navigation = useFileNavigationHistory(selectedFile, onSelectFile);

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
    if (!activeType || activeType === "side-chat" || (activeType === "diff" && passiveDiff)) return;
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
      const existing = selectExistingWorkspacePanel(current, activeType, activeTabId, selectedFile);
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
  }, [activeTabId, activeType, locale, passiveDiff, selectedFile, terminalManager.activeId, terminalManager.terminals, t]);

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
    if (!sideConversationRequest) return;
    const created = createWorkspacePanelTab("side-chat", {
      title: sideConversationRequest.title,
      sideConversation: sideConversationRequest
    }, locale);
    setTabs((current) => [...current, created]);
    setActiveTabId(created.id);
    onActiveTypeChange("side-chat");
  }, [locale, onActiveTypeChange, sideConversationRequest]);

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

  /**
   * 用选定的 SSH 主机新建远程终端标签。
   *
   * 主机密钥待确认时不建标签：连接尚未建立，
   * 先由密钥确认弹层接管，确认后再走一次本流程。
   *
   * @param hostId 目标主机标识
   * @returns 无返回值
   */
  const openSshTerminal = async (hostId: string) => {
    // 失败直接抛给主机选择对话框展示；吞掉错误会让界面看起来毫无反应
    const terminal = await terminalManager.createSshTerminal(hostId);
    if (!terminal) return;
    const created = createWorkspacePanelTab("terminal", {
      title: terminal.title || t("Terminal", "终端"),
      terminalId: terminal.id
    }, locale);
    setTabs((current) => ensureTerminalTab(current, created));
    setActiveTabId(created.id);
    onActiveTypeChange("terminal");
  };

  const addTab = async (type: WorkspacePanelAction) => {
    if (type === "side-chat") {
      onRequestSideConversation();
      return;
    }
    // SSH 需要先选主机，交给选择器处理；选定后仍落成终端面板
    if (type === "ssh") {
      setSshPickerOpen(true);
      return;
    }
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
    const existing = tabs.find((tab) => tab.type === type && (type !== "diff" || !tab.path));
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

  /**
   * 释放被关闭页签占用的终端会话。
   *
   * @param closing 即将关闭的页签
   */
  const releaseClosedTab = (closing: WorkspacePanelTab | undefined) => {
    if (closing?.type === "terminal" && closing.terminalId) {
      void terminalManager.closeTerminal(closing.terminalId);
    }
  };

  /**
   * 关闭后同步当前页签和已打开文件。
   *
   * @param next 剩余页签
   * @param preferredId 优先保持的页签
   * @param closedSelectedFile 是否关掉了当前文件页签
   */
  const settleTabs = (next: WorkspacePanelTab[], preferredId: string | null, closedSelectedFile: boolean) => {
    const preferred = next.find((tab) => tab.id === preferredId) ?? next[0] ?? null;
    setActiveTabId(preferred?.id ?? null);
    onActiveTypeChange(preferred?.type ?? null);
    if (preferred?.type === "files" && preferred.path) onSelectFile(preferred.path);
    else if (closedSelectedFile) onClearFile();
  };

  /**
   * 关闭单个页签。
   *
   * @param id 页签标识
   */
  const closeTab = (id: string) => {
    setTabs((current) => {
      const index = current.findIndex((tab) => tab.id === id);
      if (index < 0) return current;
      const closing = current[index];
      releaseClosedTab(closing);
      const next = current.filter((tab) => tab.id !== id);
      if (activeTabId === id) {
        const fallback = next[Math.max(0, index - 1)] ?? next[0] ?? null;
        settleTabs(next, fallback?.id ?? null, closing?.type === "files" && closing.path === selectedFile);
      } else if (closing?.type === "files" && closing.path === selectedFile) {
        const remainingFile = next.find((tab) => tab.type === "files" && tab.path);
        if (remainingFile?.path) onSelectFile(remainingFile.path);
        else onClearFile();
      }
      return next;
    });
  };

  /**
   * 关闭除指定页签外的全部可关闭页签。
   *
   * @param id 保留的页签标识
   */
  const closeOtherTabs = (id: string) => {
    setTabs((current) => {
      const kept = current.filter((tab) => tab.id === id || !tab.closable);
      current.filter((tab) => !kept.includes(tab)).forEach(releaseClosedTab);
      const closedSelectedFile = current.some((tab) => !kept.includes(tab) && tab.type === "files" && tab.path === selectedFile);
      settleTabs(kept, id, closedSelectedFile);
      return kept;
    });
  };

  /** 关闭全部可关闭页签。 */
  const closeAllTabs = () => {
    setTabs((current) => {
      const kept = current.filter((tab) => !tab.closable);
      current.filter((tab) => tab.closable).forEach(releaseClosedTab);
      const closedSelectedFile = current.some((tab) => tab.closable && tab.type === "files" && tab.path === selectedFile);
      settleTabs(kept, kept[0]?.id ?? null, closedSelectedFile);
      return kept;
    });
  };

  return (
    <div className="workspace-pane">
      <WorkspaceTabBar
        tabs={tabs}
        activeTabId={activeTab?.id ?? null}
        maximized={maximized}
        gitEntries={gitEntries}
        onActivate={(id) => {
          setActiveTabId(id);
          const tab = tabs.find((item) => item.id === id);
          if (!tab) return;
          onActiveTypeChange(tab.type);
          if (tab.type === "files") {
            if (tab.path) onSelectFile(tab.path);
            else onClearFile();
          }
          if (tab.type === "terminal" && tab.terminalId) terminalManager.setActiveId(tab.terminalId);
        }}
        onClose={closeTab}
        onCloseOthers={closeOtherTabs}
        onCloseAll={closeAllTabs}
        onAdd={(type) => {
          void addTab(type);
        }}
        onToggleMaximized={onToggleMaximized}
        onCollapse={onCollapse}
      />
      <ErrorBoundary key={activeTab?.id ?? "empty"} label={t("This panel failed to render", "该面板渲染失败")}>
        <Suspense fallback={<LoadingPanel />}>
          <div id="workspace-panel-content" className="pane-body" role={activeTab ? "tabpanel" : undefined} aria-labelledby={activeTab ? `workspace-tab-${activeTab.id}` : undefined}>
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
                  gitEntries={gitEntries}
                  navigation={navigation}
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
              activeTab.path
                ? <TargetedDiffPane path={activeTab.path} source={activeTab.diffSource ?? ""} />
                : <SourceControlPane />
            )}
            {activeTab?.type === "terminal" && (
              <TerminalDock terminalId={activeTab.terminalId} title={activeTab.title} error={terminalManager.error} />
            )}
            {activeTab?.type === "tasks" && <BackgroundTasksPanel />}
            {activeTab?.type === "subagents" && <SubagentWorkspace />}
            {tabs.filter((tab) => tab.type === "side-chat" && tab.sideConversation).map((tab) => (
              <div
                className="workspace-side-chat-host"
                hidden={activeTab?.id !== tab.id}
                key={tab.id}
              >
                <SideConversationPane request={tab.sideConversation!} />
              </div>
            ))}
          </div>
        </Suspense>
      </ErrorBoundary>
      <SshHostPickerDialog
        open={sshPickerOpen}
        onClose={() => setSshPickerOpen(false)}
        onPick={openSshTerminal}
      />
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
