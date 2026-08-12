import { ArrowLeft, ArrowRight, FolderTree, RefreshCw, Save } from "lucide-react";
import { MarkdownModeToggle } from "../../shared/ui/markdown-editor/markdown-mode-toggle";
import type { MarkdownEditorMode } from "../../shared/ui/markdown-editor/markdown-editor-mode";
import { EditorBreadcrumbs } from "./editor-breadcrumbs";
import { useI18n } from "../i18n/use-i18n";

/** 文件访问历史导航状态与动作。 */
export type EditorNavigation = {
  canBack: boolean;
  canForward: boolean;
  back: () => void;
  forward: () => void;
};

type EditorHeaderProps = {
  path: string;
  onSelectFile: (path: string) => void;
  /** 历史后退/前进；宿主未接入时不渲染 */
  navigation?: EditorNavigation;
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
  navigation,
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
      {navigation && (
        <span className="editor-nav" role="group" aria-label={t("File navigation history", "文件访问历史")}>
          <button
            type="button"
            disabled={!navigation.canBack}
            onClick={navigation.back}
            title={t("Back", "后退")}
            aria-label={t("Back", "后退")}
          >
            <ArrowLeft size={14} />
          </button>
          <button
            type="button"
            disabled={!navigation.canForward}
            onClick={navigation.forward}
            title={t("Forward", "前进")}
            aria-label={t("Forward", "前进")}
          >
            <ArrowRight size={14} />
          </button>
        </span>
      )}
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
