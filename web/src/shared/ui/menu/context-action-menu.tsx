import { useEffect, useId, useRef, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { Button } from "../button/button";
import type { ActionMenuItem } from "./action-menu";
import "./action-menu.css";

type ContextActionMenuProps = {
  label: string;
  x: number;
  y: number;
  items: ActionMenuItem[];
  onClose: () => void;
};

const MENU_WIDTH = 220;
const MENU_ITEM_HEIGHT = 36;

/**
 * 在指针坐标处渲染操作菜单。
 *
 * 用于右键菜单：定位按视口钳制，点击外部或按下 Escape 时关闭。
 *
 * @param props 菜单标题、坐标、动作和关闭回调
 * @returns 固定定位的操作菜单
 */
export function ContextActionMenu({ label, x, y, items, onClose }: ContextActionMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const id = useId();
  const left = Math.max(8, Math.min(x, window.innerWidth - MENU_WIDTH - 8));
  const estimatedHeight = items.length * MENU_ITEM_HEIGHT + 16;
  const top = Math.max(8, Math.min(y, window.innerHeight - estimatedHeight - 8));

  useEffect(() => {
    menuRef.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    /** 点击菜单外部时关闭。 */
    const handlePointer = (event: PointerEvent) => {
      if (event.target instanceof Node && menuRef.current?.contains(event.target)) return;
      onClose();
    };
    /** 按下 Escape 时关闭。 */
    const handleKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("pointerdown", handlePointer);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("pointerdown", handlePointer);
      document.removeEventListener("keydown", handleKey);
    };
  }, [onClose]);

  /**
   * 用方向键在可用菜单项之间移动焦点。
   *
   * @param event 菜单键盘事件
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const buttons = Array.from(menuRef.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key) || !buttons.length) return;
    event.preventDefault();
    const next = event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1
      : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length;
    buttons[next]?.focus();
  };

  return createPortal(
    <div
      id={id}
      ref={menuRef}
      role="menu"
      aria-label={label}
      className="ui-action-menu-panel"
      style={{ position: "fixed", left, top, width: MENU_WIDTH, zIndex: 120 }}
      onKeyDown={handleKeyDown}
      onContextMenu={(event) => event.preventDefault()}
    >
      {items.map((item) => (
        <div key={item.id} className={item.separator ? "ui-action-menu-group" : undefined}>
          <Button
            variant={item.danger ? "ghost-danger" : "ghost"}
            role="menuitem"
            title={item.label}
            disabled={item.disabled}
            onClick={() => {
              onClose();
              item.onSelect();
            }}
          >
            {item.icon}
            <span>{item.label}</span>
          </Button>
        </div>
      ))}
    </div>,
    document.body
  );
}
