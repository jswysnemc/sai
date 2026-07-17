import { X } from "lucide-react";
import { useState } from "react";
import type { ComposerAttachment } from "./use-composer-attachments";
import { ImageLightbox } from "../../../shared/ui/image-lightbox";

/**
 * 渲染待发送图片缩略图条。
 *
 * @param props 附件列表与删除回调
 * @returns 图片附件条
 */
export function AttachmentStrip({ attachments, onRemove }: { attachments: ComposerAttachment[]; onRemove: (id: number) => void }) {
  const [preview, setPreview] = useState<ComposerAttachment | null>(null);
  if (attachments.length === 0) return null;
  return (
    <>
      <div className="composer-attachments">
        {attachments.map((attachment) => (
          <div className="composer-attachment" key={attachment.id} title={attachment.name}>
            <button type="button" className="attachment-preview-button" onClick={() => setPreview(attachment)} aria-label={`预览 ${attachment.name}`}><img src={attachment.dataUrl} alt={attachment.name} /></button>
            <button type="button" className="attachment-remove" onClick={() => onRemove(attachment.id)} aria-label={`移除 ${attachment.name}`}><X size={13} /></button>
          </div>
        ))}
      </div>
      {preview && <ImageLightbox src={preview.dataUrl} alt={preview.name} onClose={() => setPreview(null)} />}
    </>
  );
}
