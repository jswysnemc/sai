import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { MarkdownEditor } from "../../shared/ui/markdown-editor/markdown-editor";
import { useMarkdownMode } from "../../shared/ui/markdown-editor/use-markdown-mode";
import { isDarkTheme, useTheme } from "../theme/theme";
import { useI18n } from "../i18n/use-i18n";
import { EditorHeader } from "./editor-header";
import { ImageFilePreview, isImageFile } from "./image-file-preview";
import { isMarkdownFile } from "./markdown-file-preview";
import { resolveWorkspaceImage } from "./markdown-image-url";
import { MonacoCodeEditor } from "./monaco-code-editor";
import {
  acceptSavedFile,
  applyRemoteFile,
  canSaveDocument,
  createEditorDocumentState,
  reloadRemoteFile,
  updateDocumentContent
} from "./editor-document-state";
import { useEditorGitDiff } from "./use-editor-git-diff";
import { untrackedFilePatch } from "./editor-git-decorations";
import { EditorDiffView } from "./editor-diff-view";
import { registerUnsavedEditor } from "./unsaved-editor-changes";
import { readEditorWordWrap, writeEditorWordWrap } from "./editor-word-wrap";
import { readRecentFiles, rememberRecentFile } from "./editor-recent-files";
import { FilesHome } from "./files-home";
import { OpenFileDialog } from "./open-file-dialog";
import type { EditorNavigation } from "./editor-header";
import type { FileTreeGitEntry } from "./use-workspace-git-entries";

const EMPTY_GIT_ENTRIES: ReadonlyMap<string, FileTreeGitEntry> = new Map();

type EditorPaneProps = {
  path: string | null;
  onSelectFile: (path: string) => void;
  fileTreeOpen: boolean;
  onToggleFileTree: () => void;
  /** 按工作区路径索引的 Git 状态，用于编辑器行装饰 */
  gitEntries?: ReadonlyMap<string, FileTreeGitEntry>;
  /** 文件访问历史导航 */
  navigation?: EditorNavigation;
};

/**
 * 渲染文件编辑器、路径导航和保存操作。
 *
 * Markdown 文件走两态编辑器（源码 / 可编辑预览），其余文件走 Monaco。
 *
 * @param props 当前文件、打开文件回调和文件树控制状态
 * @returns 编辑器面板
 */
export function EditorPane({ path, onSelectFile, fileTreeOpen, onToggleFileTree, gitEntries, navigation }: EditorPaneProps) {
  const { t } = useI18n();
  const { theme } = useTheme();
  const imageFile = Boolean(path && isImageFile(path));
  const markdownFile = Boolean(path && isMarkdownFile(path));
  const queryClient = useQueryClient();
  const file = useQuery({ queryKey: ["file", path], queryFn: () => api.workspace.file(path!), enabled: Boolean(path) && !imageFile });
  const [document, setDocument] = useState(() => createEditorDocumentState(path));
  const [markdownMode, setMarkdownMode] = useMarkdownMode();
  const [openFileDialog, setOpenFileDialog] = useState(false);
  const [wordWrap, setWordWrap] = useState(() => readEditorWordWrap(Boolean(path && isMarkdownFile(path))));
  const [diffView, setDiffView] = useState(false);
  const [recentFiles, setRecentFiles] = useState(readRecentFiles);
  useEffect(() => {
    if (!path) return;
    setRecentFiles(rememberRecentFile(path));
  }, [path]);
  useEffect(() => {
    setWordWrap(readEditorWordWrap(Boolean(path && isMarkdownFile(path))));
  }, [path]);
  const toggleWordWrap = () => {
    setWordWrap((current) => {
      writeEditorWordWrap(!current);
      return !current;
    });
  };
  const fileDialog = <OpenFileDialog open={openFileDialog} initialPath={path ?? ""} onSelectFile={onSelectFile} onClose={() => setOpenFileDialog(false)} />;
  const gitDiff = useEditorGitDiff(path, gitEntries ?? EMPTY_GIT_ENTRIES);
  const gitEntry = path ? (gitEntries ?? EMPTY_GIT_ENTRIES).get(path) : undefined;
  const diffPatch = gitEntry?.entry.untracked
    ? untrackedFilePatch(path ?? "", document.content)
    : gitDiff.patch;
  const hasUnsavedChanges = Boolean(document.baseline && document.content !== document.baseline.content);

  useEffect(() => {
    if (!document.path || !hasUnsavedChanges) return;
    // 1. 【工作区】【编辑保护】保存或还原后撤销登记，折叠面板时继续保护当前草稿
    return registerUnsavedEditor(document.path);
  }, [document.path, hasUnsavedChanges]);

  useEffect(() => {
    setDocument(createEditorDocumentState(path));
  }, [path]);

  useEffect(() => {
    if (!file.data) return;
    setDocument((current) => applyRemoteFile(current, file.data));
  }, [file.data]);

  const save = useMutation({
    mutationFn: () => api.workspace.save(
      path!,
      document.content,
      document.baseline?.version,
      document.baseline?.modified_at
    ),
    onSuccess: async (saved) => {
      setDocument((current) => acceptSavedFile(current, saved));
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["file", path] }),
        queryClient.invalidateQueries({ queryKey: ["workspace-diff"] }),
        // 保存即刷新 Git 状态与行装饰，标签徽标和 gutter 不等轮询
        queryClient.invalidateQueries({ queryKey: ["file-tree-git-statuses"] }),
        queryClient.invalidateQueries({ queryKey: ["git-review-diff"] }),
        queryClient.invalidateQueries({ queryKey: ["git-status"] }),
        queryClient.invalidateQueries({ queryKey: ["git-statuses"] })
      ]);
    }
  });

  /**
   * 重新读取磁盘文件并明确丢弃当前草稿。
   *
   * @returns 重载完成后的 Promise
   */
  const reload = async () => {
    const refreshed = await file.refetch();
    const remote = refreshed.data;
    if (!remote) return;
    setDocument((current) => reloadRemoteFile(applyRemoteFile(current, remote)));
  };

  if (!path) {
    return (
      <section className="editor-pane editor-pane-home">
        <FilesHome recentFiles={recentFiles} onSelectFile={onSelectFile} onBrowseFiles={() => { if (!fileTreeOpen) onToggleFileTree(); }} />
        {fileDialog}
      </section>
    );
  }

  return (
    <section className="editor-pane">
      <EditorHeader
        path={path}
        onSelectFile={onSelectFile}
        navigation={navigation}
        externalChange={document.externalChange}
        onReload={() => void reload()}
        markdownMode={markdownFile ? markdownMode : null}
        onMarkdownModeChange={setMarkdownMode}
        canSave={canSaveDocument(document) && !save.isPending}
        onSave={() => save.mutate()}
        savable={!imageFile}
        fileTreeOpen={fileTreeOpen}
        onToggleFileTree={onToggleFileTree}
        onOpenFile={() => setOpenFileDialog(true)}
        wordWrap={imageFile ? null : wordWrap}
        onToggleWordWrap={toggleWordWrap}
        diffView={imageFile ? null : diffView}
        onToggleDiffView={() => setDiffView((current) => !current)}
      />
      <div className="editor-area">
        {!imageFile && diffView && (
          <EditorDiffView
            path={path}
            gitPath={gitEntry?.entry.path ?? path}
            patch={diffPatch}
            loading={!gitEntry?.entry.untracked && gitDiff.loading}
            repoRoot={gitEntry?.repoRoot ?? ""}
            untracked={Boolean(gitEntry?.entry.untracked)}
            stagedClean={Boolean(gitEntry?.entry.staged && gitEntry.entry.worktree_status === "." && !gitEntry.entry.untracked)}
            worktreeDirty={Boolean(gitEntry && gitEntry.entry.worktree_status !== "." && !gitEntry.entry.untracked)}
          />
        )}
        {imageFile && <ImageFilePreview path={path} />}
        {!imageFile && !diffView && file.data && markdownFile && (
          <MarkdownEditor
            value={document.content}
            onChange={(next) => setDocument((current) => updateDocumentContent(current, next))}
            mode={markdownMode}
            onModeChange={setMarkdownMode}
            dark={isDarkTheme(theme)}
            wrap={wordWrap}
            resolveImageUrl={(src) => resolveWorkspaceImage(path, src)}
          />
        )}
        {!imageFile && !diffView && file.data && !markdownFile && (
          <MonacoCodeEditor
            path={path}
            value={document.content}
            onChange={(next) => setDocument((current) => updateDocumentContent(current, next))}
            loadingLabel={t("Loading editor", "加载编辑器")}
            gitLines={gitDiff.lines}
            wordWrap={wordWrap}
            onToggleWordWrap={toggleWordWrap}
          />
        )}
        {!imageFile && !diffView && file.isLoading && <div className="editor-state">{t("Loading editor", "加载编辑器")}</div>}
        {file.error && <div className="pane-error">{file.error.message}</div>}
        {save.error && <div className="pane-error">{save.error.message}</div>}
      </div>
      {fileDialog}
    </section>
  );
}
