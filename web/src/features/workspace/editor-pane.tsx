import Editor, { loader, type OnMount } from "@monaco-editor/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderTree, RefreshCw, Save } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import { useTheme } from "../theme/theme";
import { EditorBreadcrumbs } from "./editor-breadcrumbs";
import { EditorPreviewToggle } from "./editor-preview-toggle";
import { configureMonacoEnvironment } from "./monaco-environment";
import { ImageFilePreview, isImageFile } from "./image-file-preview";
import { isMarkdownFile, MarkdownFilePreview } from "./markdown-file-preview";
import { useI18n } from "../i18n/use-i18n";
import {
  acceptSavedFile,
  applyRemoteFile,
  canSaveDocument,
  createEditorDocumentState,
  reloadRemoteFile,
  updateDocumentContent
} from "./editor-document-state";

type EditorPaneProps = {
  path: string | null;
  onSelectFile: (path: string) => void;
  fileTreeOpen: boolean;
  onToggleFileTree: () => void;
};

/**
 * 渲染文件编辑器、路径导航和保存操作。
 *
 * @param props 当前文件、打开文件回调和文件树控制状态
 * @returns 编辑器面板
 */
export function EditorPane({ path, onSelectFile, fileTreeOpen, onToggleFileTree }: EditorPaneProps) {
  const { t } = useI18n();
  const { theme } = useTheme();
  const imageFile = Boolean(path && isImageFile(path));
  const markdownFile = Boolean(path && isMarkdownFile(path));
  const queryClient = useQueryClient();
  const file = useQuery({ queryKey: ["file", path], queryFn: () => api.workspace.file(path!), enabled: Boolean(path) && !imageFile });
  const [document, setDocument] = useState(() => createEditorDocumentState(path));
  const [editorReady, setEditorReady] = useState(false);
  const [preview, setPreview] = useState(false);
  const editorAreaRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  useEffect(() => {
    let active = true;
    // 1. 先注册语言 Worker，再加载 Monaco 主模块
    configureMonacoEnvironment();
    import("monaco-editor").then((monaco) => {
      loader.config({ monaco });
      if (active) setEditorReady(true);
    });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    setDocument(createEditorDocumentState(path));
    setPreview(false);
    return () => {
      editorRef.current = null;
    };
  }, [path]);
  useEffect(() => {
    if (!file.data) return;
    setDocument((current) => applyRemoteFile(current, file.data));
  }, [file.data]);

  useEffect(() => {
    const container = editorAreaRef.current;
    if (!container) return;
    let frame = 0;
    // 1. 使用实际编辑区域尺寸通知 Monaco，避免拖动网格时沿用旧宽度
    const observer = new ResizeObserver(([entry]) => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => {
        const width = Math.max(0, Math.floor(entry.contentRect.width));
        const height = Math.max(0, Math.floor(entry.contentRect.height));
        if (width > 0 && height > 0) editorRef.current?.layout({ width, height });
      });
    });
    observer.observe(container);
    return () => {
      window.cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [editorReady, path]);

  /** 保存 Monaco 实例并立即按容器尺寸布局。 */
  const handleEditorMount: OnMount = (editor) => {
    editorRef.current = editor;
    const area = editorAreaRef.current;
    if (area) editor.layout({ width: area.clientWidth, height: area.clientHeight });
  };

  const save = useMutation({
    mutationFn: () => api.workspace.save(
      path!,
      document.content,
      document.baseline?.version,
      document.baseline?.modified_at
    ),
    onSuccess: async (saved) => {
      setDocument((current) => acceptSavedFile(current, saved));
      await queryClient.invalidateQueries({ queryKey: ["file", path] });
      await queryClient.invalidateQueries({ queryKey: ["workspace-diff"] });
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
      <header className="editor-head">
        <EditorBreadcrumbs path={path} onSelectFile={onSelectFile} />
        {document.externalChange && <span className="editor-external-change">{t("File changed on disk", "磁盘内容已变化")}</span>}
        {document.externalChange && (
          <button type="button" className="editor-reload" onClick={() => void reload()} title={t("Reload file from disk", "从磁盘重新载入文件")} aria-label={t("Reload file from disk", "从磁盘重新载入文件")}>
            <RefreshCw size={14} />
          </button>
        )}
        {markdownFile && <EditorPreviewToggle preview={preview} onChange={setPreview} />}
        {!imageFile && <button type="button" className="editor-save" onClick={() => save.mutate()} disabled={!canSaveDocument(document) || save.isPending}>
          <Save size={14} /> {t("Save", "保存")}
        </button>}
        {!fileTreeOpen && (
          <button type="button" className="editor-tree-toggle" onClick={onToggleFileTree} aria-label={t("Open file tree", "打开文件树")} aria-pressed={false}>
            <FolderTree size={15} />
          </button>
        )}
      </header>
      <div className="editor-area" ref={editorAreaRef}>
        {imageFile && <ImageFilePreview path={path} />}
        {markdownFile && preview && file.data && <MarkdownFilePreview source={document.content} />}
        {file.data && editorReady && !(markdownFile && preview) && (
          <Editor
            key={path}
            path={path}
            language={languageForPath(path)}
            value={document.content}
            width="100%"
            height="100%"
            onMount={handleEditorMount}
            onChange={(value) => setDocument((current) => updateDocumentContent(current, value ?? ""))}
            theme={theme === "graphite" || theme === "ocean" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches) ? "vs-dark" : "light"}
            options={{ minimap: { enabled: false }, fontFamily: "Fira Code", fontSize: 13, lineHeight: 21, padding: { top: 12 }, automaticLayout: false, scrollBeyondLastLine: false }}
          />
        )}
        {!imageFile && (file.isLoading || !editorReady) && <div className="editor-state">{t("Loading editor", "加载编辑器")}</div>}
        {file.error && <div className="pane-error">{file.error.message}</div>}
        {save.error && <div className="pane-error">{save.error.message}</div>}
      </div>
    </section>
  );
}

function FileCodePlaceholder() {
  return <div className="file-code-placeholder">&lt;/&gt;</div>;
}

function languageForPath(path: string) {
  const extension = path.split(".").pop()?.toLowerCase();
  return ({ rs: "rust", ts: "typescript", tsx: "typescript", js: "javascript", jsx: "javascript", json: "json", md: "markdown", css: "css", html: "html", py: "python", go: "go", sh: "shell", toml: "ini", yaml: "yaml", yml: "yaml" } as Record<string, string>)[extension ?? ""] ?? "plaintext";
}
