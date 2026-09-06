import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Button } from "../button/button";
import { useAnchoredPopover } from "../popover/use-anchored-popover";
import "./action-menu.css";

export type ActionMenuItem = {
  id: string;
  label: string;
  icon?: ReactNode;
  shortcut?: string;
  disabled?: boolean;
  danger?: boolean;
  separator?: boolean;
  onSelect: () => void;
};

type ActionMenuProps = {
  label: string;
  trigger: ReactNode;
  items: ActionMenuItem[];
  className?: string;
  triggerClassName?: string;
  disabled?: boolean;
};

/**
 * 渲染统一的操作菜单，支持方向键、退出键与焦点恢复。
 * @param props 触发器、可用动作和样式
 * @returns 使用独立浮层的操作菜单
 */
export function ActionMenu({ label, trigger, items, className = "", triggerClassName = "", disabled }: ActionMenuProps) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const id = useId();
  const style = useAnchoredPopover({ open, anchorRef: triggerRef, preferredWidth: 244, minimumWidth: 200, align: "right", maxHeight: 420 });

  useEffect(() => {
    if (!open) return;
    menuRef.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    /** 关闭点击范围之外的菜单，保留目标元素的焦点。 */
    const handlePointer = (event: PointerEvent) => {
      if (!(event.target instanceof Node)) return;
      if (triggerRef.current?.contains(event.target) || menuRef.current?.contains(event.target)) return;
      setOpen(false);
    };
    document.addEventListener("pointerdown", handlePointer);
    return () => document.removeEventListener("pointerdown", handlePointer);
  }, [open]);

  /**
   * 按键盘方向移动菜单焦点，退出时回到触发器。
   * @param event 当前菜单键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const buttons = Array.from(menuRef.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      triggerRef.current?.focus();
      return;
    }
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key) || !buttons.length) return;
    event.preventDefault();
    const next = event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1
      : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length;
    buttons[next]?.focus();
  };

  return (
    <span className={`ui-action-menu ${className}`}>
      <Button
        ref={triggerRef}
        variant="ghost"
        size="icon"
        className={triggerClassName}
        aria-label={label}
        title={label}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        disabled={disabled}
        onClick={() => setOpen((current) => !current)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            setOpen(true);
          }
        }}
      >
        {trigger}
      </Button>
      {open && createPortal(
        <div id={id} ref={menuRef} role="menu" aria-label={label} className="ui-action-menu-panel" style={style} onKeyDown={handleKeyDown}>
          {items.map((item) => (
            <div key={item.id} className={item.separator ? "ui-action-menu-group" : undefined}>
              <Button
                variant={item.danger ? "ghost-danger" : "ghost"}
                role="menuitem"
                title={item.label}
                disabled={item.disabled}
                onClick={() => {
                  setOpen(false);
                  triggerRef.current?.focus();
                  item.onSelect();
                }}
              >
                {item.icon}
                <span>{item.label}</span>
                {item.shortcut && <kbd>{item.shortcut}</kbd>}
              </Button>
            </div>
          ))}
        </div>, document.body
      )}
    </span>
  );
}
