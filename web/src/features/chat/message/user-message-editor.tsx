import { ArrowRight, Paperclip, X } from "lucide-react";
import { useRef } from "react";
import type { ChangeEvent } from "react";
import { useI18n } from "../../i18n/use-i18n";
import { ComposerSurface } from "../composer/composer-surface";
import type { ComposerAttachment } from "../composer/use-composer-attachments";
import { useUserMessageEditorState } from "./use-user-message-editor-state";
import "./user-message-editor.css";

type UserMessageEditorProps = {
  /** 原消息正文 */
  content: string;
  /** 原消息图片，编辑时默认保留 */
  imageUrls: string[];
  /** 提交中时禁用全部控件 */
  busy?: boolean;
  onCancel: () => void;
  onSubmit: (content: string, imageUrls: string[]) => void;
};

/**
 * 用户消息的就地编辑器：复用主输入框外壳，省略模型与运行模式控制。
 *
 * @param props 原消息内容、忙碌状态与提交、取消回调
 * @returns 编辑表单
 */
export function UserMessageEditor({ content, imageUrls, busy, onCancel, onSubmit }: UserMessageEditorProps) {
  const { t } = useI18n();
  const editor = useUserMessageEditorState(content, imageUrls);
  const fileInputRef = useRef<HTMLInputElement>(null);

  /** 提交改写后的消息，正文与图片同时为空时不提交。 */
  const submit = () => {
    if (busy || !editor.submittable) return;
    onSubmit(editor.content.trim(), editor.imageUrls);
  };

  /**
   * 读取文件选择器中选中的图片。
   *
   * @param event 文件输入变更事件
   * @returns 无返回值
   */
  const handleFileChange = (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length === 0) return;
    void editor.addFiles(files);
  };

  const attachments: ComposerAttachment[] = editor.images.map((image) => ({
    id: image.id,
    name: t("Attached image", "已附加图片"),
    dataUrl: image.dataUrl
  }));

  return (
    <ComposerSurface
      variant="compact"
      className="composer user-message-editor"
      value={editor.content}
      historyEntries={[]}
      disabled={Boolean(busy)}
      submitDisabled={Boolean(busy) || !editor.submittable}
      autoFocus
      placeholder={t("Edit the message; press Enter to resend", "修改消息内容，Enter 重新发送")}
      attachments={attachments}
      onChange={editor.setContent}
      onPasteImages={async (files) => {
        await editor.addFiles(files);
        return undefined;
      }}
      onRemoveAttachment={editor.removeImage}
      onSubmit={submit}
    >
      {editor.error && <p className="user-message-editor-error">{editor.error}</p>}
      <div className="composer-footer user-message-editor-actions">
        <div className="composer-toolrail">
          <input ref={fileInputRef} type="file" accept="image/*" multiple hidden onChange={handleFileChange} />
          <button
            type="button"
            className="composer-icon-button"
            onClick={() => fileInputRef.current?.click()}
            disabled={busy}
            aria-label={t("Attach image", "附加图片")}
            title={t("Attach image", "附加图片")}
          >
            <Paperclip size={18} />
          </button>
        </div>
        <div className="composer-actions">
          <button
            type="button"
            className="composer-icon-button"
            onClick={onCancel}
            disabled={busy}
            aria-label={t("Cancel editing", "取消编辑")}
            title={t("Cancel editing", "取消编辑")}
          >
            <X size={15} />
          </button>
          <button
            type="button"
            className="composer-send"
            onClick={submit}
            disabled={Boolean(busy) || !editor.submittable}
            aria-label={t("Resend as a new branch", "作为新分支重新发送")}
            title={t("Undo to this message and resend as a new branch", "退回本条消息并作为新分支重新发送")}
          >
            <ArrowRight size={18} />
          </button>
        </div>
      </div>
    </ComposerSurface>
  );
}
