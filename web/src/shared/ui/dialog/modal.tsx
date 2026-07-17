import { X } from "lucide-react";
import { useEffect, useId, useRef } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../../../features/i18n/use-i18n";

type ModalProps = {
  open: boolean;
  title: string;
  description?: string;
  size?: "small" | "medium" | "large";
  children: React.ReactNode;
  footer?: React.ReactNode;
  onClose: () => void;
};

/**
 * 渲染具备遮罩、Escape 关闭和无障碍标题的通用弹层。
 *
 * @param props 弹层状态、标题、内容和关闭回调
 * @returns Portal 弹层
 */
export function Modal({ open, title, description, size = "medium", children, footer, onClose }: ModalProps) {
  const { t } = useI18n();
  const titleId = useId();
  const descriptionId = useId();
  const dialogRef = useRef<HTMLElement>(null);

  useEffect(() => {
    if (!open) return;
    const previousFocus = document.activeElement as HTMLElement | null;
    requestAnimationFrame(() => {
      const focusable = dialogRef.current?.querySelector<HTMLElement>('button, input, textarea, [tabindex]:not([tabindex="-1"])');
      focusable?.focus();
    });
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
      if (event.key !== "Tab" || !dialogRef.current) return;
      const focusable = Array.from(dialogRef.current.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])'));
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      previousFocus?.focus();
    };
  }, [open, onClose]);

  if (!open) return null;
  return createPortal(
    <div className="ui-modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section ref={dialogRef} className={`ui-modal ${size}`} role="dialog" aria-modal="true" aria-labelledby={titleId} aria-describedby={description ? descriptionId : undefined}>
        <header className="ui-modal-header">
          <div><h2 id={titleId}>{title}</h2>{description && <p id={descriptionId}>{description}</p>}</div>
          <button type="button" onClick={onClose} aria-label={t("Close dialog", "关闭对话框")}><X size={16} /></button>
        </header>
        <div className="ui-modal-body">{children}</div>
        {footer && <footer className="ui-modal-footer">{footer}</footer>}
      </section>
    </div>,
    document.body
  );
}
