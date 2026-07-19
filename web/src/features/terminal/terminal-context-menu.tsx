import { Clipboard, ClipboardPaste, MessageSquareText, ScanText } from "lucide-react";
import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n/use-i18n";

type TerminalContextMenuProps = {
  x: number;
  y: number;
  hasSelection: boolean;
  onCopy: () => void;
  onPaste: () => void;
  onSelectAll: () => void;
  onSendToChat: () => void;
  onClose: () => void;
};

/**
 * 渲染终端专用右键菜单。
 *
 * @param props 菜单坐标、选区状态和操作回调
 * @returns 固定定位的终端菜单
 */
export function TerminalContextMenu(props: TerminalContextMenuProps) {
  const { t } = useI18n();
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    /** 点击菜单外部或按下 Escape 时关闭菜单。 */
    const handlePointerDown = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) props.onClose();
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") props.onClose();
    };
    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [props.onClose]);

  const left = Math.max(8, Math.min(props.x, window.innerWidth - 196));
  const top = Math.max(8, Math.min(props.y, window.innerHeight - 174));
  return createPortal(
    <div ref={menuRef} className="terminal-context-menu" role="menu" style={{ left, top }}>
      <button type="button" role="menuitem" disabled={!props.hasSelection} onClick={props.onCopy}>
        <Clipboard size={14} /><span>{t("Copy selection", "复制选区")}</span>
      </button>
      <button type="button" role="menuitem" onClick={props.onPaste}>
        <ClipboardPaste size={14} /><span>{t("Paste", "粘贴")}</span>
      </button>
      <button type="button" role="menuitem" onClick={props.onSelectAll}>
        <ScanText size={14} /><span>{t("Select all", "全选")}</span>
      </button>
      <span className="terminal-context-menu-separator" />
      <button type="button" role="menuitem" disabled={!props.hasSelection} onClick={props.onSendToChat}>
        <MessageSquareText size={14} /><span>{t("Send selection to chat", "发送选区到聊天")}</span>
      </button>
    </div>,
    document.body
  );
}
