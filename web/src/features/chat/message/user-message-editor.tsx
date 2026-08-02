import { X } from "lucide-react";
import { useEffect, useRef } from "react";
import type { ChangeEvent, ClipboardEvent, KeyboardEvent } from "react";
import { Button } from "../../../shared/ui/button/button";
import { TextArea } from "../../../shared/ui/form/text-area";
import { useI18n } from "../../i18n/use-i18n";
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
 * 用户消息的就地编辑器：改写正文、增删图片后作为新分支重新发送。
 *
 * @param props 原消息内容、忙碌状态与提交、取消回调
 * @returns 编辑表单
 */
export function UserMessageEditor({ content, imageUrls, busy, onCancel, onSubmit }: UserMessageEditorProps) {
  const { t } = useI18n();
  const editor = useUserMessageEditorState(content, imageUrls);
  const textAreaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // 1. 进入编辑态后聚焦并把光标放到正文末尾
  useEffect(() => {
    const node = textAreaRef.current;
    if (!node) return;
    node.focus();
    node.setSelectionRange(node.value.length, node.value.length);
  }, []);

  /** 提交改写后的消息，正文与图片同时为空时不提交 */
  const submit = () => {
    if (busy || !editor.submittable) return;
    onSubmit(editor.content.trim(), editor.imageUrls);
  };

  /**
   * 处理编辑区快捷键：回车提交，Escape 退出编辑。
   *
   * @param event 键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onCancel();
      return;
    }
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      submit();
    }
  };

  /**
   * 处理粘贴：剪贴板含图片时转为附件，否则交给浏览器插入文本。
   *
   * @param event 剪贴板事件
   * @returns 无返回值
   */
  const handlePaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const files = Array.from(event.clipboardData.files).filter((file) => file.type.startsWith("image/"));
    if (files.length === 0) return;
    event.preventDefault();
    void editor.addFiles(files);
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

  return (
    <div className="user-message-editor">
      {editor.images.length > 0 && (
        <div className="user-message-editor-images">
          {editor.images.map((image) => (
            <div className="user-message-editor-image" key={image.id}>
              <img src={image.dataUrl} alt={t("Attached image", "已附加图片")} />
              <button
                type="button"
                className="user-message-editor-image-remove"
                onClick={() => editor.removeImage(image.id)}
                disabled={busy}
                aria-label={t("Remove image", "移除图片")}
                title={t("Remove image", "移除图片")}
              >
                <X size={13} />
              </button>
            </div>
          ))}
        </div>
      )}
      <TextArea
        ref={textAreaRef}
        className="user-message-editor-input"
        value={editor.content}
        disabled={busy}
        rows={3}
        spellCheck={false}
        aria-label={t("Edit message", "编辑消息")}
        placeholder={t("Edit the message; press Enter to resend", "修改消息内容，Enter 重新发送")}
        onChange={(event) => editor.setContent(event.target.value)}
        onKeyDown={handleKeyDown}
        onPaste={handlePaste}
      />
      {editor.error && <p className="user-message-editor-error">{editor.error}</p>}
      <div className="user-message-editor-actions">
        <input ref={fileInputRef} type="file" accept="image/*" multiple hidden onChange={handleFileChange} />
        <Button
          className="user-message-editor-attach"
          onClick={() => fileInputRef.current?.click()}
          disabled={busy}
        >
          {t("Add image", "添加图片")}
        </Button>
        <span className="user-message-editor-hint">
          {t("Enter to resend · Shift+Enter for a new line", "Enter 重新发送 · Shift+Enter 换行")}
        </span>
        <Button onClick={onCancel} disabled={busy}>{t("Cancel", "取消")}</Button>
        <Button variant="primary" onClick={submit} disabled={busy || !editor.submittable}>
          {t("Resend", "重新发送")}
        </Button>
      </div>
    </div>
  );
}
