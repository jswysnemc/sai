import { Check, Code2, Copy, Eye, Maximize2 } from "../../shared/ui/icons";
import { memo, useEffect, useId, useRef, useState } from "react";
import { ImageLightbox } from "../../shared/ui/image-lightbox";
import { SegmentedControl, type SegmentedControlOption } from "../../shared/ui/segmented-control";
import { useI18n } from "../i18n/use-i18n";
import { renderPeleDiagram } from "./pele-render";
import "./pele-theme.css";

/** 渲染结果缓存上限，超出时丢弃最旧条目 */
const CACHE_LIMIT = 50;

/** 放大图需要烘焙的颜色变量。自定义属性里的 var() 在 data URL 中不会继续解析。 */
const PELE_BAKE_TOKENS = ["--ink", "--ink-soft", "--paper", "--line", "--danger"] as const;

/** 模块级渲染缓存，键为源码，值为渲染出的 SVG 字符串 */
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
 * 把当前界面颜色写进 SVG，供脱离文档的放大图使用。
 *
 * @param svg SVG 字符串
 * @param host 带 Pele 变量的容器
 * @returns data:image/svg+xml URL，解析失败时返回 null
 */
function toSvgDataUrl(svg: string, host: HTMLElement): string | null {
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
  // 3. 把继承的颜色烘焙成具体值，避免放大图丢失 CSS 变量
  const computed = getComputedStyle(host);
  const baked = PELE_BAKE_TOKENS.map((token) => computed.getPropertyValue(token).trim());
  root.style.setProperty("--pele-bg", "transparent");
  root.style.setProperty("--pele-fg", baked[0] || "#202526");
  root.style.setProperty("--pele-muted", baked[1] || baked[0] || "#6d7572");
  root.style.setProperty("--pele-line", baked[0] || "#202526");
  root.style.setProperty("--pele-surface", baked[2] || "#f3f5f5");
  root.style.setProperty("--pele-surface-alt", baked[2] || "#f3f5f5");
  root.style.setProperty("--pele-border", baked[3] || "#d5dbd8");
  root.style.setProperty("--pele-error", baked[4] || "#b42318");
  root.style.removeProperty("max-width");

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
  const previewRef = useRef<HTMLDivElement>(null);
  const [svg, setSvg] = useState(() => readCache(source) ?? "");
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<"preview" | "source">("preview");
  const [copied, setCopied] = useState(false);
  const [lightboxUrl, setLightboxUrl] = useState<string | null>(null);
  const viewOptions: readonly SegmentedControlOption<"preview" | "source">[] = [
    { value: "preview", label: t("Preview", "预览"), icon: <Eye size={14} /> },
    { value: "source", label: t("Source", "源码"), icon: <Code2 size={14} /> }
  ];

  useEffect(() => {
    // 1. 命中缓存时直接同步展示，跳过渲染
    const cached = readCache(source);
    if (cached !== undefined) {
      setSvg(cached);
      setError(null);
      return;
    }
    // 2. 未命中时用 Pele 渲染。颜色走 CSS 变量，切换主题不必重绘
    let active = true;
    void renderPeleDiagram(source, `sai-mermaid-${reactId}`).then((result) => {
      if (!active) return;
      if ("svg" in result) {
        writeCache(source, result.svg);
        setSvg(result.svg);
        setError(null);
        return;
      }
      setError(result.error);
    });
    return () => {
      active = false;
    };
  }, [reactId, source]);

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
    const host = previewRef.current;
    if (!svg || !host) return;
    const url = toSvgDataUrl(svg, host);
    if (url) setLightboxUrl(url);
  };

  return (
    <div className="mermaid-wrapper">
      <div className="mermaid-toolbar">
        <span>mermaid</span>
        <SegmentedControl value={view} options={viewOptions} onChange={setView} ariaLabel={t("Mermaid display mode", "Mermaid 展示方式")} className="mermaid-view-switcher" />
        <button type="button" className="mermaid-copy" disabled={!svg || Boolean(error)} onClick={openLightbox}><Maximize2 size={14} />{t("Enlarge", "放大")}</button>
        <button type="button" className="mermaid-copy" onClick={() => void copySource()}>{copied ? <Check size={14} /> : <Copy size={14} />}{copied ? t("Copied", "已复制") : t("Copy", "复制")}</button>
      </div>
      {view === "source" || error || !svg
        ? <pre className="mermaid-source"><code>{source}</code></pre>
        : <div ref={previewRef} className="mermaid-preview pele" dangerouslySetInnerHTML={{ __html: svg }} />}
      {error && <div className="mermaid-error">{error}</div>}
      {lightboxUrl && <ImageLightbox src={lightboxUrl} alt={t("Mermaid diagram", "Mermaid 图表")} onClose={() => setLightboxUrl(null)} />}
    </div>
  );
});
