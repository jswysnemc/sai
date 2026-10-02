import { useState, type DragEvent, type FormEvent, type ReactNode } from "react";
import { TemplateManager } from "../../prompt-templates/template-manager";
import type { TemplateScope } from "../../prompt-templates/template-client";
import { AttachmentStrip } from "./attachment-strip";
import { ComposerTextarea } from "./composer-textarea";
import type { ComposerAttachment } from "./use-composer-attachments";
import "../chat-composer.css";
import "./composer-surface.css";

export type ComposerSurfaceVariant = "full" | "compact";

type ComposerSurfaceProps = {
  variant: ComposerSurfaceVariant;
  templateScope?: TemplateScope;
  onPlanCommand?: () => Promise<boolean>;
  className?: string;
  value: string;
  historyEntries: string[];
  disabled: boolean;
  submitDisabled?: boolean;
  placeholder: string;
  autoFocus?: boolean;
  respondToGlobalFocus?: boolean;
  attachments?: ComposerAttachment[];
  onChange: (value: string) => void;
  onPasteImages?: (files: File[], selectionStart: number, selectionEnd: number) => Promise<number | undefined>;
  onRemoveAttachment?: (id: number) => void;
  onSubmit: () => void;
  /** 叠在输入框上沿外侧的浮层，跟随输入框高度 */
  floating?: ReactNode;
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
  templateScope = "chat",
  onPlanCommand,
  className = "",
  value,
  historyEntries,
  disabled,
  submitDisabled = false,
  placeholder,
  autoFocus = false,
  respondToGlobalFocus = true,
  attachments,
  onChange,
  onPasteImages,
  onRemoveAttachment,
  onSubmit,
  floating,
  children
}: ComposerSurfaceProps) {
  const [dragging, setDragging] = useState(false);

  /** 统一处理表单提交和输入区 Enter 提交。 */
  const submit = (event?: FormEvent) => {
    event?.preventDefault();
    if (!submitDisabled) onSubmit();
  };

  /**
   * 整块输入壳接受拖入，图片在编辑区之外落下时也记成附件。
   *
   * @param event 拖放事件
   * @returns 无返回值
   */
  const handleDragOver = (event: DragEvent<HTMLFormElement>) => {
    if (disabled || !event.dataTransfer.types.includes("Files")) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
    setDragging(true);
  };

  /**
   * 编辑区自己的 drop 会先停掉冒泡；落到壳上的图片仍走附件。
   *
   * @param event 拖放事件
   * @returns 无返回值
   */
  const handleDrop = (event: DragEvent<HTMLFormElement>) => {
    setDragging(false);
    const files = Array.from(event.dataTransfer.files).filter((file) => file.type.startsWith("image/"));
    if (disabled || files.length === 0 || !onPasteImages) return;
    event.preventDefault();
    void onPasteImages(files, value.length, value.length);
  };

  return (
    <form
      className={`composer-surface composer-surface-${variant}${dragging ? " is-dragover" : ""}${className ? ` ${className}` : ""}`}
      onSubmit={submit}
      onDragOver={handleDragOver}
      onDragLeave={(event) => {
        if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
        setDragging(false);
      }}
      onDrop={handleDrop}
    >
      {floating}
      {attachments && onRemoveAttachment && (
        <AttachmentStrip attachments={attachments} onRemove={onRemoveAttachment} />
      )}
      <ComposerTextarea
        templateScope={templateScope}
        onPlanCommand={onPlanCommand}
        value={value}
        historyEntries={historyEntries}
        disabled={disabled}
        autoFocus={autoFocus}
        respondToGlobalFocus={respondToGlobalFocus}
        placeholder={placeholder}
        onChange={onChange}
        onPasteImages={onPasteImages}
        onSubmit={() => submit()}
      />
      {variant === "full" && <TemplateManager scope={templateScope} disabled={disabled} onApply={onChange} />}
      {children}
    </form>
  );
}
