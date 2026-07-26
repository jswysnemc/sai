import { Code2, Eye, Type } from "lucide-react";
import type { ComponentType } from "react";
import { MARKDOWN_EDITOR_MODES, type MarkdownEditorMode } from "./markdown-editor-mode";

type MarkdownModeToggleProps = {
  mode: MarkdownEditorMode;
  onChange: (mode: MarkdownEditorMode) => void;
  /** 双语文案取值函数，由调用方注入以复用各页面的 i18n 实例 */
  t: (en: string, zh: string) => string;
};

/** 各模式的图标与文案。 */
const MODE_META: Record<MarkdownEditorMode, { icon: ComponentType<{ size?: number }>; en: string; zh: string }> = {
  source: { icon: Code2, en: "Source", zh: "源码" },
  wysiwyg: { icon: Type, en: "Live", zh: "所见即所得" },
  preview: { icon: Eye, en: "Preview", zh: "预览" },
};

/**
 * 渲染 Markdown 三态切换控件。
 *
 * @param props 当前模式、变更回调与双语取值函数
 * @returns 分段切换控件
 */
export function MarkdownModeToggle({ mode, onChange, t }: MarkdownModeToggleProps) {
  return (
    <div className="markdown-mode-toggle" role="group" aria-label={t("Markdown display mode", "Markdown 显示模式")}>
      {MARKDOWN_EDITOR_MODES.map((item) => {
        const meta = MODE_META[item];
        const Icon = meta.icon;
        const label = t(meta.en, meta.zh);
        return (
          <button
            key={item}
            type="button"
            className={mode === item ? "active" : ""}
            onClick={() => onChange(item)}
            aria-pressed={mode === item}
            title={label}
          >
            <Icon size={13} />
            <span>{label}</span>
          </button>
        );
      })}
    </div>
  );
}
