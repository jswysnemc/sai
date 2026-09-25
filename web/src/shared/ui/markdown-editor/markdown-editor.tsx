import { useCallback, useRef, useState } from "react";
import { MarkdownTextEditor, type MarkdownTextEditorHandle } from "./codemirror/markdown-text-editor";
import { nextMarkdownMode, type MarkdownEditorMode } from "./markdown-editor-mode";
import { MarkdownOutline } from "./outline/markdown-outline";
import type { OutlineHeading } from "./outline/outline-model";
import "./markdown-editor.css";

type MarkdownEditorProps = {
  value: string;
  onChange: (value: string) => void;
  mode: MarkdownEditorMode;
  /** 快捷键 Ctrl+/ 切换模式时回调；缺省时快捷键不生效 */
  onModeChange?: (mode: MarkdownEditorMode) => void;
  dark: boolean;
  readOnly?: boolean;
  /** 是否自动换行，缺省为换行 */
  wrap?: boolean;
  /** 是否展示大纲，缺省展示；文档没有标题时自动隐藏 */
  outline?: boolean;
  /** 把文档里的相对图片地址解析为可加载地址；缺省只渲染绝对地址 */
  resolveImageUrl?: (src: string) => string | null;
};

/**
 * 两态 Markdown 编辑器：源码与所见即所得预览。
 *
 * 两态共用一个 CodeMirror 实例，切换只热替换装饰与主题，
 * 光标位置、滚动位置和撤销栈都保留。预览态可就地编辑，行为对齐 Typora。
 *
 * @param props 内容、变更回调、模式、主题深浅、只读状态、换行、大纲与图片解析
 * @returns 编辑区容器
 */
export function MarkdownEditor({
  value,
  onChange,
  mode,
  onModeChange,
  dark,
  readOnly = false,
  wrap = true,
  outline = true,
  resolveImageUrl,
}: MarkdownEditorProps) {
  const handleRef = useRef<MarkdownTextEditorHandle>(null);
  const [headings, setHeadings] = useState<OutlineHeading[]>([]);
  const [activeIndex, setActiveIndex] = useState(-1);
  const toggleMode = useCallback(() => onModeChange?.(nextMarkdownMode(mode)), [mode, onModeChange]);

  return (
    <div className="markdown-editor-root">
      <div className="markdown-editor-surface">
        <MarkdownTextEditor
          ref={handleRef}
          value={value}
          onChange={onChange}
          live={mode === "preview"}
          dark={dark}
          readOnly={readOnly}
          wrap={wrap}
          resolveImageUrl={resolveImageUrl}
          onToggleMode={onModeChange ? toggleMode : undefined}
          onOutline={setHeadings}
          onActiveHeading={setActiveIndex}
        />
      </div>
      {outline && (
        <MarkdownOutline
          headings={headings}
          activeIndex={activeIndex}
          onSelect={(from) => handleRef.current?.jumpTo(from)}
        />
      )}
    </div>
  );
}
