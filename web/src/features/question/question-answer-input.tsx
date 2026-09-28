import { useRef, useState, type ChangeEvent } from "react";
import { Plus } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { ComposerSurface } from "../chat/composer/composer-surface";
import { useComposerAttachments } from "../chat/composer/use-composer-attachments";
import type { Translate } from "./question-card-state";

type Props = {
  draft: string;
  attachmentKey: string;
  allowImages: boolean;
  onDraft: (value: string) => void;
  onSave: (images: string[]) => void;
  t: Translate;
};

/**
 * 使用共享编辑器回答问题，支持换行、粘贴、拖入图片和附件移除。
 * @param props 本题草稿、独立附件键、能力和提交回调
 * @returns 紧凑的多行回答输入区
 */
export function QuestionAnswerInput({ draft, attachmentKey, allowImages, onDraft, onSave, t }: Props) {
  const images = useComposerAttachments(attachmentKey);
  const fileInput = useRef<HTMLInputElement>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const hasAnswer = Boolean(draft.trim()) || (allowImages && images.attachments.length > 0);

  /** 读取图片并显示失败原因；参数为图片和选区，返回新光标位置。 */
  const addImages = async (files: File[], start: number, end: number) => {
    setError("");
    setLoading(true);
    try {
      return await images.addFiles(files, start, end);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : t("Failed to add image", "添加图片失败"));
      return undefined;
    } finally {
      setLoading(false);
    }
  };

  /** 将文件选择交给统一图片处理；参数为文件事件，返回无。 */
  const selectImages = (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length) void addImages(files, draft.length, draft.length);
  };

  return <div className="question-answer-input">
    <ComposerSurface variant="full" className="composer" value={draft} historyEntries={[]} disabled={loading}
      submitDisabled={!hasAnswer || loading} placeholder={t("Write an answer · Shift+Enter for a new line", "输入回答，Shift+Enter 换行")}
      autoFocus respondToGlobalFocus={false} onChange={onDraft}
      attachments={allowImages ? images.attachments : undefined} onPasteImages={allowImages ? addImages : undefined}
      onRemoveAttachment={images.removeAttachment} onSubmit={() => onSave(allowImages ? images.attachments.map((item) => item.dataUrl) : [])}>
      <div className="composer-footer">
        {allowImages && <>
          <input type="file" ref={fileInput} accept="image/png,image/jpeg,image/gif,image/webp" multiple hidden onChange={selectImages} />
          <Button variant="ghost" size="icon" className="composer-attach" disabled={loading} aria-label={t("Add images", "添加图片")} onClick={() => fileInput.current?.click()}><Plus size={14} /></Button>
        </>}
        {error && <span className="question-request-error" role="alert">{error}</span>}
        <Button size="small" type="submit" className="question-answer-use" disabled={!hasAnswer || loading}>{t("Use answer", "使用回答")}</Button>
      </div>
    </ComposerSurface>
  </div>;
}
