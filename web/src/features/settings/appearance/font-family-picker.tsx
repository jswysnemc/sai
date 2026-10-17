import { Check, ChevronDown } from "../../../shared/ui/icons";
import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../../i18n/use-i18n";
import { filterFontFamilies } from "../../theme/local-fonts";
import { useAnchoredPopover } from "../../../shared/ui/popover/use-anchored-popover";
import "./font-family-picker.css";

type FontFamilyPickerProps = {
  value: string;
  families: readonly string[];
  /** 留空选项上显示的默认字族说明 */
  defaultLabel: string;
  disabled?: boolean;
  /** 列表打开时读取本机字体；必须放在点击回调里，浏览器才允许访问 */
  onOpen?: () => void;
  /** 已经向系统要过字族列表 */
  loaded?: boolean;
  onChange: (value: string) => void;
};

/**
 * 【Web 设置】【字体选择】从本机字族中选择一项，留空表示使用内置默认字族。
 *
 * @param props 当前字族、候选列表、默认说明与更新回调
 * @returns 可搜索的字族选择器
 */
export function FontFamilyPicker({ value, families, defaultLabel, disabled, onOpen, loaded = false, onChange }: FontFamilyPickerProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listId = useId();
  const menuStyle = useAnchoredPopover({
    open,
    anchorRef: triggerRef,
    maxHeight: 320
  });
  const options = families.includes(value) || value === "" ? families : [value, ...families];
  const filtered = filterFontFamilies(options, query);
  const label = value || defaultLabel;

  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !menuRef.current?.contains(target)) setOpen(false);
    };
    document.addEventListener("pointerdown", handlePointerDown);
    searchRef.current?.focus();
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [open]);

  /**
   * 选中一个字族并关闭列表。
   *
   * @param next 字族名；空字符串表示默认
   * @returns 无
   */
  const choose = (next: string) => {
    onChange(next);
    setOpen(false);
    setQuery("");
  };

  return (
    <div className="font-family-picker ui-select" ref={rootRef}>
      <button
        ref={triggerRef}
        type="button"
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        disabled={disabled}
        onClick={() => {
          setOpen((visible) => !visible);
          onOpen?.();
        }}
      >
        <span className="ui-select-value" style={value ? { fontFamily: `"${value}"` } : undefined}>{label}</span>
        <ChevronDown size={14} className={open ? "open" : ""} />
      </button>
      {open && createPortal(
        <div ref={menuRef} id={listId} className="ui-select-menu font-family-picker-menu" role="listbox" style={menuStyle}>
          <input
            ref={searchRef}
            className="font-family-picker-search"
            value={query}
            placeholder={t("Search fonts", "搜索字体")}
            aria-label={t("Search fonts", "搜索字体")}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") setOpen(false);
            }}
          />
          <button type="button" role="option" aria-selected={value === ""} className={value === "" ? "active" : ""} onClick={() => choose("")}>
            <span className="ui-select-option-main"><strong>{defaultLabel}</strong></span>
            <Check size={14} />
          </button>
          {filtered.visible.map((family) => (
            <button type="button" role="option" aria-selected={family === value} className={family === value ? "active" : ""} key={family} onClick={() => choose(family)}>
              <span className="ui-select-option-main"><strong style={{ fontFamily: `"${family}"` }}>{family}</strong></span>
              <Check size={14} />
            </button>
          ))}
          {families.length === 0 && (
            <p className="font-family-picker-empty">
              {loaded
                ? t("No installed fonts were found.", "没有读到已安装字体。")
                : t("Installed fonts load when this list opens.", "打开列表时读取本机字体。")}
            </p>
          )}
          {families.length > 0 && filtered.visible.length === 0 && <p className="font-family-picker-empty">{t("No matching font", "没有匹配的字体")}</p>}
          {filtered.hidden > 0 && <p className="font-family-picker-empty">{t(`${filtered.hidden} more, keep typing`, `还有 ${filtered.hidden} 个，请继续输入`)}</p>}
        </div>,
        document.body
      )}
    </div>
  );
}
