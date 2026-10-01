import { useEffect, useRef } from "react";
import { useI18n } from "../i18n/use-i18n";
import type { BrowserSelectPopup } from "./browser-protocol";

type BrowserSelectMenuProps = {
  popup: BrowserSelectPopup;
  /** 画面显示比例，页面坐标乘以它得到面板坐标 */
  scale: number;
  onAnswer: (index: number | null) => void;
};

/**
 * 原生下拉框的选项菜单：无头浏览器不绘制原生弹出层，由面板在下拉框下方绘制。
 *
 * 支持方向键、Enter 与 Esc；点击菜单之外的位置关闭且不改动选项。
 *
 * @param props 选项、显示比例与回复回调
 * @returns 浮在画面上的选项列表
 */
export function BrowserSelectMenu({ popup, scale, onAnswer }: BrowserSelectMenuProps) {
  const { t } = useI18n();
  const listRef = useRef<HTMLUListElement>(null);

  useEffect(() => {
    const list = listRef.current;
    const current = list?.querySelector<HTMLButtonElement>(`[data-index="${popup.selected}"]`)
      ?? list?.querySelector<HTMLButtonElement>("button:not(:disabled)");
    current?.focus();
    current?.scrollIntoView({ block: "nearest" });
    /** 点击菜单外关闭。 */
    const handlePointer = (event: PointerEvent) => {
      if (list && event.target instanceof Node && !list.contains(event.target)) onAnswer(null);
    };
    window.addEventListener("pointerdown", handlePointer, true);
    return () => window.removeEventListener("pointerdown", handlePointer, true);
  }, [onAnswer, popup.selected]);

  /** 方向键在可选项之间移动焦点。 */
  const moveFocus = (step: number) => {
    const buttons = [...(listRef.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    buttons[(index + step + buttons.length) % buttons.length]?.focus();
  };

  let lastGroup = "";
  return (
    <ul
      ref={listRef}
      className="browser-select-menu"
      role="listbox"
      aria-label={t("Options", "选项")}
      style={{ left: popup.x * scale, top: (popup.y + popup.height) * scale, minWidth: popup.width * scale }}
      onKeyDown={(event) => {
        if (event.key === "ArrowDown") moveFocus(1);
        else if (event.key === "ArrowUp") moveFocus(-1);
        else if (event.key === "Escape") onAnswer(null);
        else return;
        event.preventDefault();
        event.stopPropagation();
      }}
    >
      {popup.options.map((option, index) => {
        const header = option.group && option.group !== lastGroup ? option.group : null;
        lastGroup = option.group;
        return (
          <li key={index}>
            {header && <span className="browser-select-group">{header}</span>}
            <button
              type="button"
              role="option"
              data-index={index}
              aria-selected={index === popup.selected}
              className={option.group ? "grouped" : ""}
              disabled={option.disabled}
              onClick={() => onAnswer(index)}
            >
              {option.label || " "}
            </button>
          </li>
        );
      })}
    </ul>
  );
}
