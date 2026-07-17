import { Check, Code2, Copy, Eye, Maximize2 } from "lucide-react";
import { memo, useEffect, useId, useState } from "react";
import { toDisplayError } from "../../api/api-error";
import { ImageLightbox } from "../../shared/ui/image-lightbox";
import { SegmentedControl, type SegmentedControlOption } from "../../shared/ui/segmented-control";
import { useI18n } from "../i18n/use-i18n";

/** Mermaid 初始化主题标识，参与缓存键，主题变化时不会命中旧缓存 */
const MERMAID_THEME = "neutral";

/** 渲染结果缓存上限，超出时丢弃最旧条目 */
const CACHE_LIMIT = 50;

/** 模块级渲染缓存，键为主题加源码，值为渲染出的 SVG 字符串 */
const svgCache = new Map<string, string>();

/**
 * 读取渲染缓存，同时把命中条目移到最新位置。
 *
 * @param key 缓存键
 * @returns 缓存的 SVG 字符串，未命中时返回 undefined
 */
function readCache(key: string): string | undefined {
  const value = svgCache.get(key);
  if (value !== undefined) {
    // 1. 命中后重新插入，保持 Map 迭代顺序反映最近使用
    svgCache.delete(key);
    svgCache.set(key, value);
  }
  return value;
}

/**
 * 写入渲染缓存，超出上限时按插入顺序淘汰最旧条目。
 *
 * @param key 缓存键
 * @param value 渲染出的 SVG 字符串
 * @returns 无返回值
 */
function writeCache(key: string, value: string): void {
  svgCache.delete(key);
  svgCache.set(key, value);
  if (svgCache.size > CACHE_LIMIT) {
    const oldest = svgCache.keys().next().value;
    if (oldest !== undefined) svgCache.delete(oldest);
  }
}

/**
 * 把渲染出的 SVG 序列化为可缩放的 data URL。
 *
 * @param svg SVG 字符串
 * @returns data:image/svg+xml URL，解析失败时返回 null
 */
function toSvgDataUrl(svg: string): string | null {
  // 1. 解析 SVG 文本，确认结构合法
  const doc = new DOMParser().parseFromString(svg, "image/svg+xml");
  const root = doc.documentElement;
  if (root.nodeName !== "svg") return null;

  // 2. 依据 viewBox 补齐宽高属性，保证作为图片时具有固有尺寸且缩放清晰
  const viewBox = root.getAttribute("viewBox");
  if (viewBox) {
    const parts = viewBox.split(/[\s,]+/).map(Number);
    if (parts.length === 4 && parts.every(Number.isFinite)) {
      root.setAttribute("width", String(parts[2]));
      root.setAttribute("height", String(parts[3]));
    }
  }
  // 3. 移除 Mermaid 注入的 max-width 内联样式并补上浅色背景，避免图片被压缩或在深色遮罩上不可读
  root.style.removeProperty("max-width");
  root.style.setProperty("background", "#ffffff");

  // 4. 序列化并编码为 data URL
  const serialized = new XMLSerializer().serializeToString(root);
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(serialized)}`;
}

/**
 * 渲染可在图表预览和 Mermaid 源码之间切换的内容块，支持放大查看。
 *
 * @param props Mermaid 源码
 * @returns Mermaid 图表或源码
 */
export const MermaidDiagram = memo(function MermaidDiagram({ source }: { source: string }) {
  const { t } = useI18n();
  const reactId = useId().replace(/:/g, "");
  const cacheKey = `${MERMAID_THEME}\u0000${source}`;
  const [svg, setSvg] = useState(() => readCache(cacheKey) ?? "");
  const [error, setError] = useState<Error | null>(null);
  const [view, setView] = useState<"preview" | "source">("preview");
  const [copied, setCopied] = useState(false);
  const [lightboxUrl, setLightboxUrl] = useState<string | null>(null);
  const viewOptions: readonly SegmentedControlOption<"preview" | "source">[] = [
    { value: "preview", label: t("Preview", "预览"), icon: <Eye size={13} /> },
    { value: "source", label: t("Source", "源码"), icon: <Code2 size={13} /> }
  ];

  useEffect(() => {
    // 1. 命中缓存时直接同步展示，跳过异步渲染
    const cached = readCache(cacheKey);
    if (cached !== undefined) {
      setSvg(cached);
      setError(null);
      return;
    }
    // 2. 未命中时延迟触发 mermaid 异步渲染
    let active = true;
    const timer = window.setTimeout(() => {
      import("mermaid").then(({ default: mermaid }) => {
        mermaid.initialize({ startOnLoad: false, theme: MERMAID_THEME, securityLevel: "strict" });
        return mermaid.render(`sai-mermaid-${reactId}`, source);
      })
        .then((result) => {
          // 3. 渲染成功后写入缓存并更新展示
          writeCache(cacheKey, result.svg);
          if (!active) return;
          setSvg(result.svg);
          setError(null);
        })
        .catch((reason: unknown) => {
          if (active) setError(toDisplayError(reason, "Failed to render Mermaid diagram", "Mermaid 渲染失败"));
        });
    }, 120);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [cacheKey, reactId, source, t]);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1_600);
    return () => window.clearTimeout(timer);
  }, [copied]);

  /** 复制 Mermaid 源码。 */
  const copySource = async () => {
    await navigator.clipboard.writeText(source);
    setCopied(true);
  };

  /** 把当前 SVG 序列化为 data URL 并打开放大查看层。 */
  const openLightbox = () => {
    if (!svg) return;
    const url = toSvgDataUrl(svg);
    if (url) setLightboxUrl(url);
  };

  return (
    <div className="mermaid-wrapper">
      <div className="mermaid-toolbar">
        <span>mermaid</span>
        <SegmentedControl value={view} options={viewOptions} onChange={setView} ariaLabel={t("Mermaid display mode", "Mermaid 展示方式")} className="mermaid-view-switcher" />
        <button type="button" className="mermaid-copy" disabled={!svg || Boolean(error)} onClick={openLightbox}><Maximize2 size={13} />{t("Enlarge", "放大")}</button>
        <button type="button" className="mermaid-copy" onClick={() => void copySource()}>{copied ? <Check size={13} /> : <Copy size={13} />}{copied ? t("Copied", "已复制") : t("Copy", "复制")}</button>
      </div>
      {view === "source" || error || !svg
        ? <pre className="mermaid-source"><code>{source}</code></pre>
        : <div className="mermaid-preview" dangerouslySetInnerHTML={{ __html: svg }} />}
      {error && <div className="mermaid-error">{error.message}</div>}
      {lightboxUrl && <ImageLightbox src={lightboxUrl} alt={t("Mermaid diagram", "Mermaid 图表")} onClose={() => setLightboxUrl(null)} />}
    </div>
  );
});
