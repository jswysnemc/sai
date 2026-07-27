import { forwardRef, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";
import { useAnchoredPopover } from "../../../shared/ui/popover/use-anchored-popover";

type MentionPopoverPortalProps = {
  open: boolean;
  anchorRef: RefObject<HTMLElement | null>;
  className?: string;
  ariaLabel: string;
  children: ReactNode;
};

/**
 * 将输入建议浮层挂载到页面根节点，并限制在当前视口内。
 *
 * @param props 打开状态、输入区锚点、样式类名、无障碍名称和浮层内容
 * @param ref 浮层根元素引用
 * @returns 定位后的浮层；关闭时返回 null
 */
export const MentionPopoverPortal = forwardRef<HTMLDivElement, MentionPopoverPortalProps>(
  function MentionPopoverPortal({ open, anchorRef, className, ariaLabel, children }, ref) {
    const style = useAnchoredPopover({
      open,
      anchorRef,
      align: "left",
      maxHeight: 280
    });

    if (!open) return null;
    return createPortal(
      <div
        ref={ref}
        className={`file-mention-popover${className ? ` ${className}` : ""}`}
        role="listbox"
        aria-label={ariaLabel}
        style={style}
      >
        {children}
      </div>,
      document.body
    );
  }
);
