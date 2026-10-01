import { useEffect, useRef, useState, type FormEvent } from "react";
import { ArrowLeft, ArrowRight, Globe, RotateCcw, X } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { TextInput } from "../../shared/ui/form/text-input";
import { useI18n } from "../i18n/use-i18n";
import type { BrowserClientMessage, BrowserState } from "./browser-protocol";

type BrowserToolbarProps = {
  state: BrowserState | null;
  disabled: boolean;
  onSend: (message: BrowserClientMessage) => void;
};

/**
 * 浏览器面板顶部工具栏：后退、前进、刷新或停止，以及地址栏。
 *
 * 地址栏获得焦点后不再跟随页面地址刷新，避免覆盖用户正在输入的内容。
 *
 * @param props 浏览器状态、是否禁用与发送方法
 * @returns 工具栏
 */
export function BrowserToolbar({ state, disabled, onSend }: BrowserToolbarProps) {
  const { t } = useI18n();
  const [draft, setDraft] = useState("");
  const editingRef = useRef(false);
  const currentUrl = state?.url === "about:blank" ? "" : state?.url ?? "";

  useEffect(() => {
    if (!editingRef.current) setDraft(currentUrl);
  }, [currentUrl]);

  /** 提交地址栏：交给服务端补全协议或转为搜索。 */
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const url = draft.trim();
    if (!url) return;
    onSend({ type: "navigate", url });
    editingRef.current = false;
    (event.currentTarget.querySelector("input") as HTMLInputElement | null)?.blur();
  };

  return (
    <div className="browser-toolbar">
      <Button
        variant="ghost"
        size="icon"
        aria-label={t("Back", "后退")}
        title={t("Back", "后退")}
        disabled={disabled || !state?.can_go_back}
        onClick={() => onSend({ type: "back" })}
      >
        <ArrowLeft size={14} aria-hidden />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label={t("Forward", "前进")}
        title={t("Forward", "前进")}
        disabled={disabled || !state?.can_go_forward}
        onClick={() => onSend({ type: "forward" })}
      >
        <ArrowRight size={14} aria-hidden />
      </Button>
      {state?.loading ? (
        <Button
          variant="ghost"
          size="icon"
          aria-label={t("Stop loading", "停止加载")}
          title={t("Stop loading", "停止加载")}
          disabled={disabled}
          onClick={() => onSend({ type: "stop" })}
        >
          <X size={14} aria-hidden />
        </Button>
      ) : (
        <Button
          variant="ghost"
          size="icon"
          aria-label={t("Reload", "刷新")}
          title={t("Reload", "刷新")}
          disabled={disabled}
          onClick={() => onSend({ type: "reload" })}
        >
          <RotateCcw size={14} aria-hidden />
        </Button>
      )}
      <form className="browser-address" onSubmit={submit} role="search">
        <Globe size={12} aria-hidden className="browser-address-icon" />
        <TextInput
          value={draft}
          disabled={disabled}
          spellCheck={false}
          autoComplete="off"
          aria-label={t("Address or search", "地址或搜索")}
          placeholder={t("Enter an address or search", "输入地址或搜索内容")}
          onFocus={(event) => {
            editingRef.current = true;
            event.currentTarget.select();
          }}
          onBlur={() => {
            editingRef.current = false;
            setDraft(currentUrl);
          }}
          onKeyDown={(event) => {
            if (event.key !== "Escape") return;
            setDraft(currentUrl);
            event.currentTarget.blur();
          }}
          onChange={(event) => setDraft(event.target.value)}
        />
      </form>
    </div>
  );
}
