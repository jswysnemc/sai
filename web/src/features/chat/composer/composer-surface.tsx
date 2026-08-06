import type { FormEvent, ReactNode } from "react";
import { AttachmentStrip } from "./attachment-strip";
import { ComposerTextarea } from "./composer-textarea";
import type { ComposerAttachment } from "./use-composer-attachments";
import "../chat-composer.css";
import "./composer-surface.css";

export type ComposerSurfaceVariant = "full" | "compact";

type ComposerSurfaceProps = {
  variant: ComposerSurfaceVariant;
  className?: string;
  value: string;
  historyEntries: string[];
  disabled: boolean;
  submitDisabled?: boolean;
  placeholder: string;
  autoFocus?: boolean;
  attachments?: ComposerAttachment[];
  onChange: (value: string) => void;
  onPasteImages: (files: File[], selectionStart: number, selectionEnd: number) => Promise<number | undefined>;
  onRemoveAttachment?: (id: number) => void;
  onSubmit: () => void;
  children: ReactNode;
};

/**
 * 提供统一的 Composer 表单外壳，完整和精简模式只通过插槽区别外围控制。
 *
 * @param props 变体、输入状态、附件操作和底部控制内容
 * @returns 可复用的 Composer 表单
 */
export function ComposerSurface({
  variant,
  className = "",
  value,
  historyEntries,
  disabled,
  submitDisabled = false,
  placeholder,
  autoFocus = false,
  attachments,
  onChange,
  onPasteImages,
  onRemoveAttachment,
  onSubmit,
  children
}: ComposerSurfaceProps) {
  /** 统一处理表单提交和输入区 Enter 提交。 */
  const submit = (event?: FormEvent) => {
    event?.preventDefault();
    if (!submitDisabled) onSubmit();
  };

  return (
    <form
      className={`composer-surface composer-surface-${variant}${className ? ` ${className}` : ""}`}
      onSubmit={submit}
    >
      {attachments && onRemoveAttachment && (
        <AttachmentStrip attachments={attachments} onRemove={onRemoveAttachment} />
      )}
      <ComposerTextarea
        value={value}
        historyEntries={historyEntries}
        disabled={disabled}
        autoFocus={autoFocus}
        placeholder={placeholder}
        onChange={onChange}
        onPasteImages={onPasteImages}
        onSubmit={() => submit()}
      />
      {children}
    </form>
  );
}
