import { Download, LoaderCircle } from "lucide-react";
import { useMemo } from "react";
import { useI18n } from "../../i18n/use-i18n";
import { LightboxImage } from "../../../shared/ui/image-lightbox";
import { ToolPanel } from "./layout/tool-panel";

type ImageGenerationToolViewProps = {
  output: string;
};

type GeneratedImage = {
  src: string;
  alt: string;
  fileName?: string;
};

/**
 * 渲染生图工具的图片结果，并兼容统一摘要和旧插件返回文本。
 *
 * 参数:
 * - output: 工具结果 JSON 或供应商返回的图片文本
 *
 * 返回:
 * - 图片画廊；无法识别时返回空结果提示
 */
export function ImageGenerationToolView({ output }: ImageGenerationToolViewProps) {
  const { t } = useI18n();
  const images = useMemo(() => parseGeneratedImages(output), [output]);
  if (!output.trim()) {
    return (
      <ToolPanel className="image-generation-tool-view">
        <div className="image-generation-waiting" aria-busy="true">
          <LoaderCircle size={15} className="animate-spin" aria-hidden />
          <span>{t("Generating image", "正在生成图片")}</span>
        </div>
      </ToolPanel>
    );
  }
  if (images.length === 0) {
    return (
      <ToolPanel className="image-generation-tool-view">
        <p className="image-generation-empty">{t("Image result is unavailable.", "图片结果暂不可用。")}</p>
      </ToolPanel>
    );
  }
  return (
    <ToolPanel className="image-generation-tool-view">
      <div className="image-generation-grid">
        {images.map((image, index) => (
          <figure className="image-generation-item" key={image.src + "-" + index}>
            <LightboxImage className="image-generation-preview" src={image.src} alt={image.alt} />
            <figcaption>
              <span>{image.alt}</span>
              <a
                className="image-generation-download"
                href={image.src}
                download={image.fileName}
                aria-label={t("Download image", "下载图片")}
                title={t("Download image", "下载图片")}
              >
                <Download size={14} aria-hidden />
              </a>
            </figcaption>
          </figure>
        ))}
      </div>
    </ToolPanel>
  );
}

/**
 * 从统一 JSON、data URL、远程 URL 或 HTML 图片块提取安全图片地址。
 *
 * 参数:
 * - output: 工具结果文本
 *
 * 返回:
 * - 可交给 img 元素的图片地址
 */
function parseGeneratedImages(output: string): GeneratedImage[] {
  const parsed = tryParseJson(output);
  const candidates: Array<{ src: unknown; name?: unknown }> = [];
  if (parsed && Array.isArray(parsed.images)) {
    for (const image of parsed.images) {
      if (typeof image === "object" && image !== null) {
        const record = image as Record<string, unknown>;
        pushImageRecordCandidates(candidates, record);
      } else {
        candidates.push({ src: image });
      }
    }
  }
  if (parsed) pushImageRecordCandidates(candidates, parsed);
  for (const key of ["data", "data_url", "src", "html", "content", "output"]) {
    const value = parsed?.[key];
    if (typeof value !== "string") continue;
    if (key === "html" || key === "content" || key === "output") {
      const match = value.match(/<img\b[^>]*\bsrc\s*=\s*["']([^"']+)["']/i);
      if (match) candidates.push({ src: match[1] });
    } else {
      candidates.push({ src: value });
    }
  }
  if (candidates.length === 0) {
    const html = /<img\b[^>]*\bsrc\s*=\s*["']([^"']+)["']/gi;
    for (const match of output.matchAll(html)) candidates.push({ src: match[1] });
    const dataUrl = output.match(/data:image\/[a-z0-9.+-]+;base64,[a-z0-9+/=_-]+/i)?.[0];
    if (dataUrl) candidates.push({ src: dataUrl });
    const directUrl = output.match(/https?:\/\/[^\s"'<>]+/i)?.[0];
    if (directUrl) candidates.push({ src: directUrl });
  }
  return candidates.flatMap((candidate, index) => {
    const src = typeof candidate.src === "string" && isSafeImageSource(candidate.src) ? candidate.src : null;
    if (!src) return [];
    return [{
      src,
      alt: "Generated image " + (index + 1),
      fileName: typeof candidate.name === "string" ? candidate.name : undefined
    }];
  });
}

/**
 * 将统一图片记录转换为浏览器可访问的候选地址。
 *
 * @param candidates 待追加的图片候选集合
 * @param record 服务端返回的图片记录
 * @returns 无返回值
 */
function pushImageRecordCandidates(
  candidates: Array<{ src: unknown; name?: unknown }>,
  record: Record<string, unknown>
): void {
  const fileName = typeof record.file_name === "string"
    ? record.file_name
    : localImageFileName(record.local_path ?? record.path ?? record.file_path);
  for (const key of ["url", "data_url", "src"]) {
    const value = record[key];
    if (typeof value === "string") candidates.push({ src: value, name: fileName });
  }
  const localUrl = localImageUrl(record.local_path ?? record.path ?? record.file_path);
  if (localUrl) candidates.push({ src: localUrl, name: fileName });
}

/**
 * 从服务端返回的本地路径中提取缓存文件名，兼容 Windows 和 Unix 分隔符。
 *
 * @param value 本地图片路径
 * @returns 缓存文件名；路径无效时返回 undefined
 */
function localImageFileName(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  return value.split(/[\\/]/u).filter(Boolean).at(-1);
}

/**
 * 将本地缓存路径映射到公开的图片媒体路由。
 *
 * @param value 本地图片路径
 * @returns 浏览器可请求的媒体地址；文件名不安全时返回 undefined
 */
function localImageUrl(value: unknown): string | undefined {
  const fileName = localImageFileName(value);
  if (!fileName || !/^[A-Za-z0-9][A-Za-z0-9.-]*$/u.test(fileName)) return undefined;
  return `/api/generated-images/${encodeURIComponent(fileName)}`;
}

/**
 * 限制图片来源协议，阻止工具输出注入脚本 URL。
 *
 * 参数:
 * - src: 待校验图片地址
 *
 * 返回:
 * - 是否允许渲染
 */
function isSafeImageSource(src: string): boolean {
  return src.startsWith("/api/generated-images/")
    || src.startsWith("data:image/")
    || src.startsWith("https://")
    || src.startsWith("http://");
}

function tryParseJson(output: string): Record<string, unknown> | null {
  try {
    const value: unknown = JSON.parse(output);
    return typeof value === "object" && value !== null && !Array.isArray(value)
      ? value as Record<string, unknown>
      : null;
  } catch {
    return null;
  }
}
