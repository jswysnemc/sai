import { Image as ImageIcon } from "lucide-react";
import { useI18n } from "../../i18n/use-i18n";

/**
 * 队列行的内容预览。
 *
 * 队列关心的是「排在第几、写了什么、要不要提前」，不是消息本身的呈现，
 * 因此这里不复用聊天气泡：气泡自带不对称圆角、投影和 fit-content 宽度，
 * 短消息会缩成一枚窄胶囊，长消息又会把行撑高，都不适合并排成列表。
 *
 * @param position 队列中的位置，从 0 开始
 * @param content 消息正文
 * @param imageUrls 随消息发送的图片
 * @returns 单行内容预览
 */
export function QueuedMessagePreview({
  position,
  content,
  imageUrls
}: {
  position: number;
  content: string;
  imageUrls: string[];
}) {
  const { t } = useI18n();
  const text = content.trim();

  return (
    <div className="queued-message-preview">
      <span className="queued-message-index" aria-hidden="true">
        {position + 1}
      </span>
      {imageUrls.length > 0 ? (
        <span
          className="queued-message-preview-images"
          title={t(`${imageUrls.length} images attached`, `附带 ${imageUrls.length} 张图片`)}
        >
          <ImageIcon size={11} aria-hidden="true" />
          {imageUrls.length}
        </span>
      ) : null}
      <p title={text || undefined}>
        {text || <em>{t("Images only", "仅图片")}</em>}
      </p>
    </div>
  );
}
