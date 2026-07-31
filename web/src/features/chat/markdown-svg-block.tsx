import { Check, Code2, Copy, Eye, Maximize2 } from "lucide-react";
import { memo, useEffect, useState } from "react";
import { ImageLightbox } from "../../shared/ui/image-lightbox";
import { SegmentedControl, type SegmentedControlOption } from "../../shared/ui/segmented-control";
import { useI18n } from "../i18n/use-i18n";
import { MarkdownCodeBlock } from "./markdown-code-block";
import { toSvgDataUrl } from "./markdown-svg";
import { SyntaxHighlighter } from "./syntax-highlighter";

/**
 * 渲染可在图形预览和 SVG 源码之间切换的 Markdown 内容块。
 *
 * SVG 通过 img 的图片上下文加载，不把任意标签注入页面 DOM。
 *
 * @param props SVG 源码
 * @returns SVG 图形预览或源码
 */
export const MarkdownSvgBlock = memo(function MarkdownSvgBlock({ source }: { source: string }) {
  const { t } = useI18n();
  const imageUrl = toSvgDataUrl(source);
  const [view, setView] = useState<"preview" | "source">("preview");
  const [copied, setCopied] = useState(false);
  const [invalid, setInvalid] = useState(false);
  const [lightboxOpen, setLightboxOpen] = useState(false);
  const viewOptions: readonly SegmentedControlOption<"preview" | "source">[] = [
    { value: "preview", label: t("Preview", "预览"), icon: <Eye size={13} /> },
    { value: "source", label: t("Source", "源码"), icon: <Code2 size={13} /> }
  ];

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1_600);
    return () => window.clearTimeout(timer);
  }, [copied]);

  /** 复制 SVG 原始内容。 */
  const copySource = async () => {
    await navigator.clipboard.writeText(source);
    setCopied(true);
  };

  if (!imageUrl) return <MarkdownCodeBlock language="svg" source={source} />;

  return (
    <div className="markdown-svg-block">
      <div className="markdown-svg-toolbar">
        <span>svg</span>
        <SegmentedControl
          value={view}
          options={viewOptions}
          onChange={setView}
          ariaLabel={t("SVG display mode", "SVG 展示方式")}
          className="markdown-svg-view-switcher"
        />
        <button type="button" disabled={invalid} onClick={() => setLightboxOpen(true)}>
          <Maximize2 size={13} />
          {t("Enlarge", "放大")}
        </button>
        <button type="button" onClick={() => void copySource()}>
          {copied ? <Check size={13} /> : <Copy size={13} />}
          {copied ? t("Copied", "已复制") : t("Copy", "复制")}
        </button>
      </div>
      {view === "source" || invalid ? (
        <pre className="markdown-svg-source"><SyntaxHighlighter language="xml" source={source} /></pre>
      ) : (
        <div className="markdown-svg-preview">
          <img
            className="markdown-svg-preview-image"
            src={imageUrl}
            alt={t("SVG diagram", "SVG 图形")}
            onError={() => setInvalid(true)}
          />
        </div>
      )}
      {lightboxOpen && !invalid && (
        <ImageLightbox src={imageUrl} alt={t("SVG diagram", "SVG 图形")} onClose={() => setLightboxOpen(false)} />
      )}
    </div>
  );
});
