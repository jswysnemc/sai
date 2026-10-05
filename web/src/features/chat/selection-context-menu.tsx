import { useState, type MouseEvent, type ReactNode } from "react";
import { ContextActionMenu } from "../../shared/ui/menu/context-action-menu";
import type { ActionMenuItem } from "../../shared/ui/menu/action-menu";
import { useI18n } from "../i18n/use-i18n";
import { dispatchComposerText } from "./composer/composer-events";

type SelectionContextMenuProps = {
  children: ReactNode;
  className?: string;
  fallbackText?: string;
};

/**
 * 【聊天】【选区菜单】拦截原生右键，把选中文本复制或写入输入区。
 * @param props 容器内容和无选区时的回退文本
 * @returns 带定制菜单的容器
 */
export function SelectionContextMenu({ children, className, fallbackText }: SelectionContextMenuProps) {
  const { t } = useI18n();
  const [menu, setMenu] = useState<{ x: number; y: number; text: string } | null>(null);

  /**
   * 打开选区菜单；无选区时使用回退文本。
   * @param event 右键事件
   * @returns 无
   */
  const openMenu = (event: MouseEvent<HTMLElement>) => {
    const selected = window.getSelection()?.toString().trim() || fallbackText?.trim() || "";
    if (!selected) return;
    event.preventDefault();
    event.stopPropagation();
    setMenu({ x: event.clientX, y: event.clientY, text: selected });
  };

  const items: ActionMenuItem[] = menu ? [
    { id: "copy", label: t("Copy", "复制"), onSelect: () => { void navigator.clipboard.writeText(menu.text); } },
    { id: "send", label: t("Send to composer", "发送到输入区"), onSelect: () => dispatchComposerText(menu.text) },
    { id: "quote", label: t("Quote in composer", "引用到输入区"), onSelect: () => dispatchComposerText(menu.text, true) }
  ] : [];

  return (
    <div className={className} style={{ display: "contents" }} onContextMenu={openMenu}>
      {children}
      {menu && (
        <ContextActionMenu
          label={t("Selection actions", "选区操作")}
          x={menu.x}
          y={menu.y}
          items={items}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}
