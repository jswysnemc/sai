import type { ImageAspectRatio } from "../chat/image-generation/image-generation-options";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染与目标比例一致的生成等待画面。
 *
 * @param props 画面比例
 * @returns 等待占位
 */
export function ImageWorkbenchWaiting({ aspectRatio }: { aspectRatio: ImageAspectRatio }) {
  const { t } = useI18n();
  return (
    <div className="image-wait" style={{ aspectRatio: aspectRatio.replace(":", " / ") }} aria-busy="true">
      <span className="image-wait-sheen" aria-hidden />
      <span className="image-wait-label">{t("Generating image", "正在生成图片")}</span>
    </div>
  );
}
