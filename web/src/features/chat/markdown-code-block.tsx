import { Check, Copy, FileCode2, WrapText } from "../../shared/ui/icons";
import { memo, useEffect, useState } from "react";
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
export const MarkdownCodeBlock = memo(function MarkdownCodeBlock({
  language,
  source,
  style = DEFAULT_MARKDOWN_STYLE_PREFERENCES.codeBlock
}: MarkdownCodeBlockProps) {
  const { t } = useI18n();
  const [copied, setCopied] = useState(false);
  const [wrapped, setWrapped] = useState(style.wrapLongLines);
  const showHeader = style.showLanguageLabel || style.showCopyButton;

  useEffect(() => {
    setWrapped(style.wrapLongLines);
  }, [style.wrapLongLines]);

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
    <div className="markdown-code-block" data-wrapped={wrapped ? "true" : "false"}>
      {showHeader && (
        <div className="markdown-code-head">
          {style.showLanguageLabel && (
            <span className="markdown-code-lang">
              <FileCode2 size={13} />
              {language || "text"}
            </span>
          )}
          <span className="markdown-code-actions">
            <button
              type="button"
              aria-pressed={wrapped}
              aria-label={wrapped ? t("Disable line wrap", "关闭自动换行") : t("Wrap long lines", "长行换行")}
              title={wrapped ? t("Disable line wrap", "关闭自动换行") : t("Wrap long lines", "长行换行")}
              onClick={() => setWrapped((current) => !current)}
            >
              <WrapText size={13} />
            </button>
            {style.showCopyButton && (
              <button
                type="button"
                aria-label={copied ? t("Copied", "已复制") : t("Copy", "复制")}
                title={copied ? t("Copied", "已复制") : t("Copy", "复制")}
                onClick={() => void copySource()}
              >
                {copied ? <Check size={13} /> : <Copy size={13} />}
              </button>
            )}
          </span>
        </div>
      )}
      <pre><SyntaxHighlighter language={language} source={source} showLineNumbers={style.lineNumbers} /></pre>
    </div>
  );
});
