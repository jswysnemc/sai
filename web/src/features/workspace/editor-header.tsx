import { FolderTree, RefreshCw, Save } from "lucide-react";
import { MarkdownModeToggle } from "../../shared/ui/markdown-editor/markdown-mode-toggle";
import type { MarkdownEditorMode } from "../../shared/ui/markdown-editor/markdown-editor-mode";
import { EditorBreadcrumbs } from "./editor-breadcrumbs";
import { useI18n } from "../i18n/use-i18n";

type EditorHeaderProps = {
  path: string;
  onSelectFile: (path: string) => void;
  /** 磁盘内容已变化 */
  externalChange: boolean;
  onReload: () => void;
  /** 为 null 表示当前文件不是 Markdown */
  markdownMode: MarkdownEditorMode | null;
  onMarkdownModeChange: (mode: MarkdownEditorMode) => void;
  canSave: boolean;
  onSave: () => void;
  /** 为 false 表示不支持保存（如图片） */
  savable: boolean;
  fileTreeOpen: boolean;
  onToggleFileTree: () => void;
};

/**
 * 渲染编辑器头部工具栏。
 *
 * @param props 路径导航、外部变更提示、模式切换与保存相关的状态和回调
 * @returns 编辑器头部
 */
export function EditorHeader({
  path,
  onSelectFile,
  externalChange,
  onReload,
  markdownMode,
  onMarkdownModeChange,
  canSave,
  onSave,
  savable,
  fileTreeOpen,
  onToggleFileTree,
}: EditorHeaderProps) {
  const { t } = useI18n();
  return (
    <header className="editor-head">
      <EditorBreadcrumbs path={path} onSelectFile={onSelectFile} />
      {externalChange && (
        <span className="editor-external-change">{t("File changed on disk", "磁盘内容已变化")}</span>
      )}
      {externalChange && (
        <button
          type="button"
          className="editor-reload"
          onClick={onReload}
          title={t("Reload file from disk", "从磁盘重新载入文件")}
          aria-label={t("Reload file from disk", "从磁盘重新载入文件")}
        >
          <RefreshCw size={14} />
        </button>
      )}
      {markdownMode && (
        <MarkdownModeToggle mode={markdownMode} onChange={onMarkdownModeChange} t={t} />
      )}
      {savable && (
        <button type="button" className="editor-save" onClick={onSave} disabled={!canSave}>
          <Save size={14} /> {t("Save", "保存")}
        </button>
      )}
      {!fileTreeOpen && (
        <button
          type="button"
          className="editor-tree-toggle"
          onClick={onToggleFileTree}
          aria-label={t("Open file tree", "打开文件树")}
          aria-pressed={false}
        >
          <FolderTree size={15} />
        </button>
      )}
    </header>
  );
}
