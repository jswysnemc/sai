import type { ReactNode } from "react";
import { MarkdownTextEditor } from "./codemirror/markdown-text-editor";
import { isEditableMode, type MarkdownEditorMode } from "./markdown-editor-mode";
import "./markdown-editor.css";

type MarkdownEditorProps = {
  value: string;
  onChange: (value: string) => void;
  mode: MarkdownEditorMode;
  dark: boolean;
  readOnly?: boolean;
  /** 预览模式的渲染结果，由调用方注入以复用各自的 Markdown 渲染器 */
  renderPreview: (source: string) => ReactNode;
};

/**
 * 三态 Markdown 编辑器。
 *
 * 源码与所见即所得共用一个 CodeMirror 实例，切换只热替换装饰与主题，
 * 光标位置和撤销栈都保留；预览模式交给调用方注入的渲染器。
 *
 * @param props 内容、变更回调、模式、主题深浅、只读状态与预览渲染函数
 * @returns 编辑区容器
 */
export function MarkdownEditor({
  value,
  onChange,
  mode,
  dark,
  readOnly = false,
  renderPreview,
}: MarkdownEditorProps) {
  if (mode === "preview") {
    return <div className="markdown-editor-preview">{renderPreview(value)}</div>;
  }
  return (
    <div className="markdown-editor-surface">
      <MarkdownTextEditor
        value={value}
        onChange={onChange}
        live={mode === "wysiwyg"}
        dark={dark}
        readOnly={readOnly || !isEditableMode(mode)}
      />
    </div>
  );
}
