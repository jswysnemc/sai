import { Check, Copy } from "lucide-react";
import { useEffect, useState } from "react";
import { SyntaxHighlighter } from "./syntax-highlighter";
import { useI18n } from "../i18n/use-i18n";
import {
  DEFAULT_MARKDOWN_STYLE_PREFERENCES,
  type MarkdownCodeBlockStylePreferences
} from "../markdown/markdown-style-preferences";

type MarkdownCodeBlockProps = {
  language?: string;
  source: string;
  style?: MarkdownCodeBlockStylePreferences;
};

/**
 * 渲染带语言标签和复制操作的 Markdown 代码块。
 *
 * @param props 代码语言和源代码
 * @returns Markdown 代码块
 */
export function MarkdownCodeBlock({
  language,
  source,
  style = DEFAULT_MARKDOWN_STYLE_PREFERENCES.codeBlock
}: MarkdownCodeBlockProps) {
  const { t } = useI18n();
  const [copied, setCopied] = useState(false);
  const showHeader = style.showLanguageLabel || style.showCopyButton;

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1_600);
    return () => window.clearTimeout(timer);
  }, [copied]);

  /** 复制代码块原始内容。 */
  const copySource = async () => {
    await navigator.clipboard.writeText(source);
    setCopied(true);
  };

  return (
    <div className="markdown-code-block">
      {showHeader && (
        <div className="markdown-code-head">
          {style.showLanguageLabel && <span>{language || "text"}</span>}
          {style.showCopyButton && (
            <button type="button" onClick={() => void copySource()}>
              {copied ? <Check size={13} /> : <Copy size={13} />}
              <span>{copied ? t("Copied", "已复制") : t("Copy", "复制")}</span>
            </button>
          )}
        </div>
      )}
      <pre><SyntaxHighlighter language={language} source={source} showLineNumbers={style.lineNumbers} /></pre>
    </div>
  );
}
