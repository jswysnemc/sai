import { ChevronDown, Search, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { formatRelativeTime } from "../../shared/format-relative-time";
import { Button } from "../../shared/ui/button/button";
import { useAnchoredPopover } from "../../shared/ui/popover/use-anchored-popover";
import { useI18n } from "../i18n/use-i18n";
import type { WorkspacePanelTab } from "./workspace-tab";
import { workspacePanelTitle } from "./workspace-panel-options";

type WorkspaceTabSwitcherProps = {
  tabs: WorkspacePanelTab[];
  activeTabId: string | null;
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
  iconFor: (tab: WorkspacePanelTab) => ReactNode;
};

/**
 * 列出已打开的页签，可搜索，并在每一行关闭。
 *
 * @param props 页签和激活、关闭回调
 * @returns 页签切换菜单
 */
export function WorkspaceTabSwitcher({ tabs, activeTabId, onActivate, onClose, iconFor }: WorkspaceTabSwitcherProps) {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [now, setNow] = useState(() => Date.now());
  const openedAt = useRef(new Map<string, number>());
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const style = useAnchoredPopover({ open, anchorRef: triggerRef, preferredWidth: 280, minimumWidth: 240, align: "left", maxHeight: 360 });

  useEffect(() => {
    const seen = openedAt.current;
    const alive = new Set(tabs.map((tab) => tab.id));
    for (const tab of tabs) {
      if (!seen.has(tab.id)) seen.set(tab.id, Date.now());
    }
    for (const id of seen.keys()) {
      if (!alive.has(id)) seen.delete(id);
    }
  }, [tabs]);

  useEffect(() => {
    if (!open) return;
    const timer = window.setInterval(() => setNow(Date.now()), 30_000);
    const handlePointer = (event: PointerEvent) => {
      if (!(event.target instanceof Node)) return;
      if (triggerRef.current?.contains(event.target) || menuRef.current?.contains(event.target)) return;
      setOpen(false);
    };
    document.addEventListener("pointerdown", handlePointer);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("pointerdown", handlePointer);
    };
  }, [open]);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return tabs;
    return tabs.filter((tab) => (tab.title || tab.path || "").toLowerCase().includes(needle));
  }, [query, tabs]);

  return (
    <span className="workspace-tab-switcher">
      <Button
        ref={triggerRef}
        variant="ghost"
        size="icon"
        aria-label={t("Open tabs", "打开的标签页")}
        aria-expanded={open}
        title={t("Open tabs", "打开的标签页")}
        onClick={() => setOpen((value) => !value)}
      >
        <ChevronDown size={15} />
      </Button>
      {open && createPortal(
        <div ref={menuRef} className="workspace-tab-switcher-menu" style={style}>
          <label>
            <Search size={13} aria-hidden />
            <input
              autoFocus
              value={query}
              placeholder={t("Search tabs...", "搜索标签页...")}
              aria-label={t("Search tabs", "搜索标签页")}
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
          <p>{t("Open tabs", "打开的标签页")}</p>
          <ul>
            {visible.map((tab) => {
              const title = tab.title || workspacePanelTitle(tab.type, t);
              return (
                <li key={tab.id} className={tab.id === activeTabId ? "is-active" : ""}>
                  <button type="button" onClick={() => { onActivate(tab.id); setOpen(false); }}>
                    {iconFor(tab)}
                    <span>{title}</span>
                  </button>
                  <small>{formatRelativeTime(openedAt.current.get(tab.id) ?? now, locale, now)}</small>
                  {tab.closable && (
                    <Button variant="ghost" size="icon" aria-label={t(`Close ${title}`, `关闭 ${title}`)} title={title} onClick={() => onClose(tab.id)}>
                      <X size={12} />
                    </Button>
                  )}
                </li>
              );
            })}
          </ul>
        </div>,
        document.body
      )}
    </span>
  );
}
