import { createContext, memo, useContext, type ReactNode } from "react";
import ReactMarkdown, { defaultUrlTransform, type Components } from "react-markdown";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import { MarkdownCodeBlock } from "./markdown-code-block";
import { MermaidDiagram } from "./mermaid-diagram";
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
const remarkPlugins = [remarkGfm, remarkMath];
const rehypePlugins = [rehypeKatex];
const inlineAtomContext = createContext<readonly ReactNode[]>([]);
const INLINE_ATOM_PATTERN = /^sai-atom-(\d+)$/u;

/** 模块级组件映射常量，保证子组件在父组件重渲染时不被卸载重建 */
const markdownComponents: Components = {
  code({ className, children, ...props }) {
    const language = /language-(\w+)/.exec(className ?? "")?.[1];
    const text = String(children).replace(/\n$/, "");
    const inlineAtoms = useContext(inlineAtomContext);
    const atomIndex = !language ? INLINE_ATOM_PATTERN.exec(text)?.[1] : undefined;
    if (atomIndex !== undefined) {
      return <>{inlineAtoms[Number(atomIndex)] ?? children}</>;
    }
    if (language === "mermaid") return <MermaidDiagram source={text} />;
    if (language || text.includes("\n")) return <MarkdownCodeBlock language={language} source={text} />;
    return <code className="inline-code" {...props}>{children}</code>;
  },
  a({ children, ...props }) {
    return <a {...props} target="_blank" rel="noreferrer">{children}</a>;
  },
  table({ children }) {
    return <div className="markdown-table-wrap"><table>{children}</table></div>;
  },
  img({ alt, ...props }) {
    return <img {...props} alt={alt ?? ""} loading="lazy" />;
  }
};

/**
 * 渲染支持 GFM、数学公式、代码块和 Mermaid 的 Markdown 内容。
 *
 * @param props Markdown 源文本
 * @returns Markdown 内容
 */
export const MarkdownRenderer = memo(function MarkdownRenderer({
  source,
  inlineAtoms = []
}: {
  source: string;
  inlineAtoms?: readonly ReactNode[];
}) {
  return (
    <inlineAtomContext.Provider value={inlineAtoms}>
      <div className="markdown-body">
        <ReactMarkdown
          remarkPlugins={remarkPlugins}
          rehypePlugins={rehypePlugins}
          urlTransform={transformUrl}
          components={markdownComponents}
        >
          {source}
        </ReactMarkdown>
      </div>
    </inlineAtomContext.Provider>
  );
});
