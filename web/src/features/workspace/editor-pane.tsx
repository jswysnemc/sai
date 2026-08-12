import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderTree } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { MarkdownEditor } from "../../shared/ui/markdown-editor/markdown-editor";
import type { MarkdownEditorMode } from "../../shared/ui/markdown-editor/markdown-editor-mode";
import { isDarkTheme, useTheme } from "../theme/theme";
import { useI18n } from "../i18n/use-i18n";
import { EditorHeader } from "./editor-header";
import { ImageFilePreview, isImageFile } from "./image-file-preview";
import { isMarkdownFile, MarkdownFilePreview } from "./markdown-file-preview";
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
 * Markdown 文件走三态编辑器（源码 / 所见即所得 / 预览），其余文件走 Monaco。
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
  const [markdownMode, setMarkdownMode] = useState<MarkdownEditorMode>("wysiwyg");
  const gitLines = useEditorGitDiff(path, gitEntries ?? EMPTY_GIT_ENTRIES);

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
      <section className="editor-pane">
        <header className="editor-head editor-head-empty">
          <span>{t("No file open", "未打开文件")}</span>
          {!fileTreeOpen && (
            <button type="button" className="editor-tree-toggle" onClick={onToggleFileTree} aria-label={t("Open file tree", "打开文件树")} aria-pressed={false}>
              <FolderTree size={15} />
            </button>
          )}
        </header>
        <div className="editor-empty"><FileCodePlaceholder /><p>{t("Select a text file from the file tree", "从文件树选择文本文件")}</p></div>
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
      />
      <div className="editor-area">
        {imageFile && <ImageFilePreview path={path} />}
        {!imageFile && file.data && markdownFile && (
          <MarkdownEditor
            value={document.content}
            onChange={(next) => setDocument((current) => updateDocumentContent(current, next))}
            mode={markdownMode}
            dark={isDarkTheme(theme)}
            renderPreview={(source) => <MarkdownFilePreview source={source} />}
          />
        )}
        {!imageFile && file.data && !markdownFile && (
          <MonacoCodeEditor
            path={path}
            value={document.content}
            onChange={(next) => setDocument((current) => updateDocumentContent(current, next))}
            loadingLabel={t("Loading editor", "加载编辑器")}
            gitLines={gitLines}
          />
        )}
        {!imageFile && file.isLoading && <div className="editor-state">{t("Loading editor", "加载编辑器")}</div>}
        {file.error && <div className="pane-error">{file.error.message}</div>}
        {save.error && <div className="pane-error">{save.error.message}</div>}
      </div>
    </section>
  );
}

function FileCodePlaceholder() {
  return <div className="file-code-placeholder">&lt;/&gt;</div>;
}
