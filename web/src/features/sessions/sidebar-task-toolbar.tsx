import { useLayoutEffect, useRef, useState } from "react";
import { useI18n } from "../i18n/use-i18n";
import type { SidebarBrowseMode } from "./sidebar-browse";

type SidebarTaskToolbarProps = {
  mode: SidebarBrowseMode;
  onChange: (mode: SidebarBrowseMode) => void;
};

/**
 * 在会话、工作区和文件树之间切换。
 *
 * @param props 当前浏览和切换回调
 * @returns 左栏浏览条
 */
export function SidebarTaskToolbar({ mode, onChange }: SidebarTaskToolbarProps) {
  const { t } = useI18n();
  const listRef = useRef<HTMLDivElement>(null);
  const [pill, setPill] = useState({ x: 0, width: 0 });
  const tabs: Array<{ id: SidebarBrowseMode; label: string }> = [
    { id: "sessions", label: t("Sessions", "会话") },
    { id: "workspaces", label: t("Workspaces", "工作区") },
    { id: "files", label: t("Files", "文件树") }
  ];

  useLayoutEffect(() => {
    const selected = listRef.current?.querySelector<HTMLElement>("[aria-selected='true']");
    if (!selected) return;
    setPill({ x: selected.offsetLeft, width: selected.offsetWidth });
  }, [mode, tabs.map((tab) => tab.label).join("|")]);

  return (
    <div className="sidebar-task-toolbar">
      <div ref={listRef} className="sidebar-view-capsule" role="tablist" aria-label={t("Sidebar browse", "侧栏浏览")}>
        {tabs.map((tab) => (
          <button key={tab.id} type="button" role="tab" aria-selected={mode === tab.id} onClick={() => onChange(tab.id)}>
            {tab.label}
          </button>
        ))}
        <span aria-hidden style={{ width: pill.width, transform: `translateX(${pill.x}px)` }} />
      </div>
    </div>
  );
}
