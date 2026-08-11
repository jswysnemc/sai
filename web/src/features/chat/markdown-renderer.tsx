import { createContext, memo, useContext, useDeferredValue, type ReactNode } from "react";
import ReactMarkdown, { defaultUrlTransform, type Components } from "react-markdown";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import { LightboxImage } from "../../shared/ui/image-lightbox";
import { MarkdownCodeBlock } from "./markdown-code-block";
import { MermaidDiagram } from "./mermaid-diagram";
import { MarkdownSvgBlock } from "./markdown-svg-block";
import { remarkSvgBlocks } from "./markdown-svg";
import { ToolFileReference } from "./tool-renderers/tool-file-reference";
import {
  DEFAULT_MARKDOWN_STYLE_PREFERENCES,
  type MarkdownStylePreferences
} from "../markdown/markdown-style-preferences";
import { useMarkdownStylePreferences } from "../markdown/markdown-style-store";
import "./markdown-renderer.css";

/**
 * 放行 data:image URL，其余交给默认清洗规则。
 *
 * @param url 原始 URL
 * @returns 允许渲染的 URL
 */
function transformUrl(url: string): string {
  if (url.startsWith("data:image/")) return url;
  return defaultUrlTransform(url);
}

/** 模块级插件常量，避免每次渲染创建新数组导致 ReactMarkdown 重新解析 */
const remarkPlugins = [remarkGfm, remarkMath, remarkSvgBlocks];
const rehypePlugins = [rehypeKatex];
/** 流式阶段跳过数学/SVG 插件，显著降低每个 delta 的解析成本 */
const streamingRemarkPlugins = [remarkGfm];
const streamingRehypePlugins: typeof rehypePlugins = [];
const inlineAtomContext = createContext<readonly ReactNode[]>([]);
const markdownStyleContext = createContext<MarkdownStylePreferences>(DEFAULT_MARKDOWN_STYLE_PREFERENCES);
const INLINE_ATOM_PATTERN = /^sai-atom-(\d+)$/u;

/** 识别不需要文件系统查询即可判断为项目路径的短代码片段。 */
function looksLikeProjectFilePath(value: string): boolean {
  const path = value.trim().replaceAll("\\", "/");
  if (!path || path.includes(" ") || path.includes("://") || path.startsWith("#")) return false;
  if (!/^(?:\.?\.?\/)?[A-Za-z0-9_@.-]+(?:\/[A-Za-z0-9_@.-]+)+$|^[A-Za-z0-9_@.-]+\.[A-Za-z0-9]{1,12}$/u.test(path)) return false;
  const basename = path.split("/").at(-1) ?? "";
  return /\.[A-Za-z0-9]{1,12}$/u.test(basename) && !/^\d+(?:\.\d+)+$/u.test(basename);
}

/** 模块级组件映射常量，保证子组件在父组件重渲染时不被卸载重建 */
const markdownComponents: Components = {
  code({ className, children, ...props }) {
    const language = /language-(\w+)/.exec(className ?? "")?.[1]?.toLowerCase();
    const text = String(children).replace(/\n$/, "");
    const inlineAtoms = useContext(inlineAtomContext);
    const markdownStyle = useContext(markdownStyleContext);
    const atomIndex = !language ? INLINE_ATOM_PATTERN.exec(text)?.[1] : undefined;
    if (atomIndex !== undefined) {
      return <>{inlineAtoms[Number(atomIndex)] ?? children}</>;
    }
    if (language === "mermaid") return <MermaidDiagram source={text} />;
    if (language === "svg") return <MarkdownSvgBlock source={text} />;
    if (language || text.includes("\n")) {
      return <MarkdownCodeBlock language={language} source={text} style={markdownStyle.codeBlock} />;
    }
    if (looksLikeProjectFilePath(text)) {
      return (
        <ToolFileReference path={text} label={String(children)} className="inline-file-reference" />
      );
    }
    return <code className="inline-code" {...props}>{children}</code>;
  },
  a({ children, ...props }) {
    return <a {...props} target="_blank" rel="noreferrer">{children}</a>;
  },
  table({ children }) {
    return <div className="markdown-table-wrap"><table>{children}</table></div>;
  },
  img({ alt, src, className }) {
    if (!src) return null;
    return <LightboxImage className={className} src={src} alt={alt ?? ""} />;
  }
};

/**
 * 渲染支持 GFM、数学公式、代码块、SVG 和 Mermaid 的 Markdown 内容。
 *
 * @param props Markdown 源文本
 * @returns Markdown 内容
 */
export const MarkdownRenderer = memo(function MarkdownRenderer({
  source,
  inlineAtoms = [],
  stylePreferences,
  streaming = false
}: {
  source: string;
  inlineAtoms?: readonly ReactNode[];
  stylePreferences?: MarkdownStylePreferences;
  /** 流式输出时延后解析并使用轻量插件集 */
  streaming?: boolean;
}) {
  const storedStyle = useMarkdownStylePreferences();
  const style = stylePreferences ?? storedStyle.preferences;
  // 流式高频更新时让出紧急渲染；定稿后仍用最新 source 立即渲染
  const deferredSource = useDeferredValue(source);
  const renderSource = streaming ? deferredSource : source;

  return (
    <markdownStyleContext.Provider value={style}>
      <inlineAtomContext.Provider value={inlineAtoms}>
        <div
          className="markdown-body"
          data-table-border={style.table.borderStyle}
          data-table-density={style.table.density}
          data-table-width={style.table.fullWidth ? "full" : "content"}
          data-table-striped={String(style.table.stripedRows)}
          data-table-header={String(style.table.headerBackground)}
          data-table-wrap={String(style.table.wrapCells)}
          data-code-wrap={String(style.codeBlock.wrapLongLines)}
          data-code-border={String(style.codeBlock.showBorder)}
          data-code-font-size={style.codeBlock.fontSize}
          data-code-tab-size={style.codeBlock.tabSize}
          data-code-max-height={style.codeBlock.maxHeight}
        >
          <ReactMarkdown
            remarkPlugins={streaming ? streamingRemarkPlugins : remarkPlugins}
            rehypePlugins={streaming ? streamingRehypePlugins : rehypePlugins}
            urlTransform={transformUrl}
            components={markdownComponents}
          >
            {renderSource}
          </ReactMarkdown>
        </div>
      </inlineAtomContext.Provider>
    </markdownStyleContext.Provider>
  );
});
