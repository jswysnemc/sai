import { MarkdownRenderer } from "../chat/markdown-renderer";

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
      <MarkdownRenderer source={source} />
    </div>
  );
}

/**
 * 判断文件是否支持 Markdown 预览。
 *
 * @param path 文件路径
 * @returns 是否为 Markdown 文件
 */
export function isMarkdownFile(path: string): boolean {
  return /\.(md|markdown)$/i.test(path);
}
