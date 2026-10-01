import { Plus, X } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import type { BrowserClientMessage, BrowserTab } from "./browser-protocol";

type BrowserTabStripProps = {
  tabs: BrowserTab[];
  disabled: boolean;
  onSend: (message: BrowserClientMessage) => void;
};

/**
 * 浏览器内部标签条：切换、关闭与新建标签页。
 *
 * 只有一个标签时也保留新建按钮，标签名过长时截断并在 title 中给出全文。
 *
 * @param props 标签列表、是否禁用与发送方法
 * @returns 标签条
 */
export function BrowserTabStrip({ tabs, disabled, onSend }: BrowserTabStripProps) {
  const { t } = useI18n();
  return (
    <div className="browser-tab-strip" role="tablist" aria-label={t("Browser tabs", "浏览器标签页")}>
      {tabs.map((tab) => {
        const title = tab.title || (tab.url === "about:blank" ? t("New tab", "新标签页") : tab.url);
        return (
          <div key={tab.id} className={`browser-tab${tab.active ? " active" : ""}`}>
            <button
              type="button"
              role="tab"
              aria-selected={tab.active}
              className="browser-tab-title"
              title={`${title}\n${tab.url}`}
              disabled={disabled}
              onClick={() => !tab.active && onSend({ type: "switch_tab", id: tab.id })}
            >
              {title}
            </button>
            <button
              type="button"
              className="browser-tab-close"
              aria-label={t(`Close ${title}`, `关闭 ${title}`)}
              disabled={disabled}
              onClick={() => onSend({ type: "close_tab", id: tab.id })}
            >
              <X size={12} aria-hidden />
            </button>
          </div>
        );
      })}
      <Button
        variant="ghost"
        size="icon"
        className="browser-tab-new"
        aria-label={t("New tab", "新建标签页")}
        title={t("New tab", "新建标签页")}
        disabled={disabled}
        onClick={() => onSend({ type: "new_tab" })}
      >
        <Plus size={14} aria-hidden />
      </Button>
    </div>
  );
}
