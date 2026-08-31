import { Image as ImageIcon, X } from "lucide-react";
import { useI18n } from "../../i18n/use-i18n";

/**
 * 渲染排队消息附带的图片缩略图，编辑时可逐张移除。
 *
 * @param imageUrls 图片地址
 * @param onRemove 可选移除回调；缺省时只预览
 * @returns 缩略图条
 */
export function QueuedImageStrip({
  imageUrls,
  onRemove
}: {
  imageUrls: string[];
  onRemove?: (index: number) => void;
}) {
  const { t } = useI18n();
  if (imageUrls.length === 0) return null;

  return (
    <div
      className="queued-message-thumbs"
      title={t(`${imageUrls.length} images attached`, `附带 ${imageUrls.length} 张图片`)}
    >
      {imageUrls.map((url, index) => (
        <span key={`${url}-${index}`} className="queued-message-thumb">
          <img src={url} alt="" />
          {onRemove && (
            <button
              type="button"
              className="queued-message-thumb-remove"
              onClick={() => onRemove(index)}
              aria-label={t(`Remove image ${index + 1}`, `删除第 ${index + 1} 张图片`)}
              title={t("Remove image", "删除图片")}
            >
              <X size={10} />
            </button>
          )}
        </span>
      ))}
    </div>
  );
}

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
        <>
          <QueuedImageStrip imageUrls={imageUrls} />
          <span className="queued-message-preview-images" aria-hidden="true">
            <ImageIcon size={11} />
            {imageUrls.length}
          </span>
        </>
      ) : null}
      <p title={text || undefined}>
        {text || <em>{t("Images only", "仅图片")}</em>}
      </p>
    </div>
  );
}
