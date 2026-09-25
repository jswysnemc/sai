import type { EditorView } from "@codemirror/view";
import { Check, ChevronRight } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import type { FormatAction } from "../editor-format-actions";
import type { MenuEntry } from "./context-menu-entries";
import { FormatToolbar } from "./format-toolbar";
import "./editor-menus.css";

type MarkdownContextMenuProps = {
  view: EditorView;
  x: number;
  y: number;
  /** 顶部图标条的动作分组；表格内等不适用的场景传空数组 */
  toolbar: FormatAction[][];
  entries: MenuEntry[];
  label: string;
  onClose: () => void;
};

/** 视口边缘留白（像素）。 */
const EDGE = 8;

/**
 * 按视口钳制浮层位置，放不下时向左或向上翻转。
 *
 * @param element 浮层元素
 * @param x 期望的左上角横坐标
 * @param y 期望的左上角纵坐标
 * @param flipFrom 横向放不下时改为以该横坐标为右边界，用于子菜单向左展开
 * @returns 无
 */
function clampIntoViewport(element: HTMLElement, x: number, y: number, flipFrom?: number): void {
  const { width, height } = element.getBoundingClientRect();
  let left = x;
  if (left + width > window.innerWidth - EDGE) left = flipFrom !== undefined ? flipFrom - width : window.innerWidth - width - EDGE;
  const top = Math.min(y, window.innerHeight - height - EDGE);
  element.style.left = `${Math.max(EDGE, left)}px`;
  element.style.top = `${Math.max(EDGE, top)}px`;
}

type MenuListProps = {
  entries: MenuEntry[];
  /** 选中叶子菜单项时回调 */
  onSelect: (entry: MenuEntry) => void;
  /** 子菜单关闭时把焦点交还父级 */
  onBack?: () => void;
};

/**
 * 渲染一层菜单列表，支持方向键导航与悬停展开子菜单。
 *
 * @param props 菜单项、选中回调与返回父级回调
 * @returns 菜单列表
 */
function MenuList({ entries, onSelect, onBack }: MenuListProps) {
  const listRef = useRef<HTMLDivElement>(null);
  const [openId, setOpenId] = useState<string | null>(null);
  const [anchor, setAnchor] = useState<DOMRect | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  /**
   * 展开或收起子菜单；悬停时稍作延迟，斜向移动鼠标经过其他项时不会误切换。
   *
   * @param entry 目标菜单项
   * @param element 菜单项按钮
   * @param delay 延迟毫秒数
   * @returns 无
   */
  const openSubmenu = (entry: MenuEntry | null, element: HTMLElement | null, delay: number) => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      setOpenId(entry?.children ? entry.id : null);
      setAnchor(entry?.children && element ? element.getBoundingClientRect() : null);
    }, delay);
  };

  useEffect(() => () => clearTimeout(timer.current), []);

  /**
   * 键盘导航：上下移动、右键展开子菜单、左键返回父级。
   *
   * @param event 键盘事件
   * @returns 无
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const buttons = Array.from(listRef.current?.querySelectorAll<HTMLButtonElement>(":scope > button:not(:disabled)") ?? []);
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      const next = (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length;
      buttons[next]?.focus();
    } else if (event.key === "ArrowRight" && index >= 0) {
      const entry = entries.find((item) => item.id === buttons[index].dataset.id);
      if (!entry?.children) return;
      event.preventDefault();
      event.stopPropagation();
      openSubmenu(entry, buttons[index], 0);
    } else if (event.key === "ArrowLeft" && onBack) {
      event.preventDefault();
      event.stopPropagation();
      onBack();
    }
  };

  const open = entries.find((entry) => entry.id === openId);
  return (
    <div ref={listRef} className="md-menu-list" role="menu" onKeyDown={handleKeyDown}>
      {entries.map((entry) => {
        const Icon = entry.icon;
        return (
          <button
            key={entry.id}
            type="button"
            role="menuitem"
            data-id={entry.id}
            className={[
              "md-menu-item",
              entry.separator ? "has-separator" : "",
              entry.danger ? "is-danger" : "",
              entry.id === openId ? "is-open" : "",
            ].join(" ")}
            disabled={entry.disabled}
            aria-haspopup={entry.children ? "menu" : undefined}
            aria-expanded={entry.children ? entry.id === openId : undefined}
            onMouseEnter={(event) => openSubmenu(entry, event.currentTarget, entry.children ? 80 : 160)}
            onClick={(event) => {
              if (entry.children) {
                openSubmenu(entry, event.currentTarget, 0);
                return;
              }
              onSelect(entry);
            }}
          >
            <span className="md-menu-icon">{entry.checked ? <Check size={13} /> : Icon ? <Icon size={13} /> : null}</span>
            <span className="md-menu-label">{entry.label}</span>
            {entry.shortcut && <kbd>{entry.shortcut}</kbd>}
            {entry.children && <ChevronRight size={12} className="md-menu-chevron" />}
          </button>
        );
      })}
      {open?.children && anchor && (
        <Submenu
          entries={open.children}
          anchor={anchor}
          onSelect={onSelect}
          onBack={() => {
            setOpenId(null);
            listRef.current?.querySelector<HTMLButtonElement>(`[data-id="${open.id}"]`)?.focus();
          }}
        />
      )}
    </div>
  );
}

type SubmenuProps = {
  entries: MenuEntry[];
  anchor: DOMRect;
  onSelect: (entry: MenuEntry) => void;
  onBack: () => void;
};

/**
 * 渲染子菜单浮层，贴在父级菜单项右侧，放不下时翻到左侧。
 *
 * @param props 菜单项、父级菜单项位置与回调
 * @returns 子菜单浮层
 */
function Submenu({ entries, anchor, onSelect, onBack }: SubmenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (!ref.current) return;
    clampIntoViewport(ref.current, anchor.right + 2, anchor.top - 4, anchor.left - 2);
  }, [anchor]);
  return (
    <div ref={ref} className="md-menu-panel md-submenu" style={{ position: "fixed" }}>
      <MenuList entries={entries} onSelect={onSelect} onBack={onBack} />
    </div>
  );
}

/**
 * Markdown 编辑区的右键菜单：顶部格式图标条 + 与位置相关的动作列表。
 *
 * @param props 编辑器视图、坐标、图标条、菜单项与关闭回调
 * @returns 固定定位的菜单浮层
 */
export function MarkdownContextMenu({ view, x, y, toolbar, entries, label, onClose }: MarkdownContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    if (ref.current) clampIntoViewport(ref.current, x, y);
    ref.current?.querySelector<HTMLButtonElement>(".md-menu-item:not(:disabled)")?.focus({ preventScroll: true });
  }, [x, y]);

  useEffect(() => {
    /** 点击菜单外部时关闭。 */
    const handlePointer = (event: PointerEvent) => {
      if (event.target instanceof Node && ref.current?.contains(event.target)) return;
      onClose();
    };
    /** 按下 Escape 时关闭并把焦点交还编辑器。 */
    const handleKey = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "Escape") return;
      onClose();
      view.focus();
    };
    document.addEventListener("pointerdown", handlePointer, true);
    document.addEventListener("keydown", handleKey);
    window.addEventListener("blur", onClose);
    return () => {
      document.removeEventListener("pointerdown", handlePointer, true);
      document.removeEventListener("keydown", handleKey);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose, view]);

  return createPortal(
    <div
      ref={ref}
      className="md-menu-panel md-context-menu"
      role="dialog"
      aria-label={label}
      style={{ position: "fixed", left: x, top: y }}
      onContextMenu={(event) => event.preventDefault()}
    >
      {toolbar.length > 0 && <FormatToolbar view={view} groups={toolbar} onDone={onClose} />}
      {entries.length > 0 && (
        <MenuList
          entries={entries}
          onSelect={(entry) => {
            // 先关菜单并把焦点交还编辑器，再执行动作；表格动作随后会把焦点移进单元格
            onClose();
            view.focus();
            entry.run?.();
          }}
        />
      )}
    </div>,
    document.body
  );
}
