import { ArrowLeft, ArrowRight, MoreHorizontal, RefreshCw } from "lucide-react";
import { useState } from "react";
import { ActionMenu } from "../../shared/ui/menu/action-menu";
import { MarkdownModeToggle } from "../../shared/ui/markdown-editor/markdown-mode-toggle";
import type { MarkdownEditorMode } from "../../shared/ui/markdown-editor/markdown-editor-mode";
import { EditorBreadcrumbs } from "./editor-breadcrumbs";
import { EditorContextMenu } from "./editor-context-menu";
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
  onOpenFile?: () => void;
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
  /** 为 null 时不提供换行按钮（如图片） */
  wordWrap: boolean | null;
  onToggleWordWrap: () => void;
  /** 为 null 时不提供差异模式（如图片） */
  diffView: boolean | null;
  onToggleDiffView: () => void;
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
  onOpenFile,
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
  wordWrap,
  onToggleWordWrap,
  diffView,
  onToggleDiffView,
}: EditorHeaderProps) {
  const { t } = useI18n();
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  return (
    <header
      className="editor-head"
      onContextMenu={(event) => {
        event.preventDefault();
        setMenu({ x: event.clientX, y: event.clientY });
      }}
    >
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
      <ActionMenu
        className="editor-more"
        label={t("Editor actions", "编辑器操作")}
        trigger={<MoreHorizontal size={15} />}
        triggerClassName="editor-more-trigger"
        items={[
          ...(savable ? [{ id: "save", label: t("Save", "保存"), shortcut: "Ctrl+S", disabled: !canSave, onSelect: onSave }] : []),
          ...(onOpenFile ? [{ id: "open", label: t("Open file", "打开文件"), onSelect: onOpenFile }] : []),
          ...(diffView !== null ? [{ id: "diff", label: diffView ? t("Exit diff view", "退出差异") : t("Diff view", "差异视图"), separator: true, onSelect: onToggleDiffView }] : []),
          ...(wordWrap !== null ? [{ id: "wrap", label: wordWrap ? t("Disable word wrap", "关闭自动换行") : t("Enable word wrap", "开启自动换行"), onSelect: onToggleWordWrap }] : []),
          { id: "tree", label: fileTreeOpen ? t("Close file tree", "关闭文件树") : t("Open file tree", "打开文件树"), onSelect: onToggleFileTree }
        ]}
      />
      {menu && (
        <EditorContextMenu
          x={menu.x}
          y={menu.y}
          path={path}
          wordWrap={wordWrap}
          savable={savable}
          canSave={canSave}
          onToggleWordWrap={onToggleWordWrap}
          onSave={onSave}
          onClose={() => setMenu(null)}
        />
      )}
    </header>
  );
}
