import { MarkdownRenderer } from "../chat/markdown-renderer";
import { DEFAULT_MARKDOWN_STYLE_PREFERENCES } from "../markdown/markdown-style-preferences";

type MarkdownFilePreviewProps = {
  source: string;
};

/**
 * 使用聊天区域的通用 Markdown 渲染器预览文件内容。
 *
 * @param props Markdown 源文本
 * @returns 可滚动的 Markdown 文件预览
 */
export function MarkdownFilePreview({ source }: MarkdownFilePreviewProps) {
  return (
    <div className="editor-markdown-preview">
      <MarkdownRenderer source={source} stylePreferences={FILE_PREVIEW_STYLE} />
    </div>
  );
}

/**
 * 判断文件是否支持 Markdown 预览。
 *
 * @param path 文件路径
 * @returns 是否为 Markdown 文件
 */
const FILE_PREVIEW_STYLE = {
  ...DEFAULT_MARKDOWN_STYLE_PREFERENCES,
  preset: "document" as const,
  codeBlock: {
    ...DEFAULT_MARKDOWN_STYLE_PREFERENCES.codeBlock,
    showLanguageLabel: false,
    showCopyButton: false,
    lineNumbers: false,
    fontSize: "small" as const
  }
};

export function isMarkdownFile(path: string): boolean {
  return /\.(md|markdown)$/i.test(path);
}
