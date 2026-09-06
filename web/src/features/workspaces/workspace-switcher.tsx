import { useMutation, useQuery } from "@tanstack/react-query";
import { Check, ChevronDown, ChevronsLeftRight, FolderGit2, FolderOpen, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "../../api/client";
import { localizeApiMessage } from "../../api/api-error";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { useAnchoredPopover } from "../../shared/ui/popover/use-anchored-popover";
import { ServerDirectoryDialog } from "./server-directory-dialog";
import "./workspace-switcher.css";
import { useI18n } from "../i18n/use-i18n";
import { switchWithTerminalConfirm } from "./workspace-switch-confirmation";

export { switchWithTerminalConfirm } from "./workspace-switch-confirmation";

/**
 * 渲染紧凑工作区入口、最近工作区和服务端目录浏览器。
 *
 * @returns 工作区选择器
 */
export function WorkspaceSwitcher() {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(false);
  const [browserOpen, setBrowserOpen] = useState(false);
  // 触发器在「名称」与「完整路径」两种展示之间切换
  const [showPath, setShowPath] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const confirm = useConfirm();
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list });
  const active = workspaces.data?.workspaces.find((workspace) => workspace.id === workspaces.data.active_id);
  const activeName = active ? localizeApiMessage(active.name, locale) : t("Workspace", "工作区");
  const switchWorkspace = useMutation({
    mutationFn: (id: string) => switchWithTerminalConfirm(id, confirm, t),
    onSuccess: (switched) => { if (switched) window.location.reload(); }
  });
  const menuStyle = useAnchoredPopover({ open, anchorRef: triggerRef, preferredWidth: 520, minimumWidth: 240, maxHeight: 560 });

  useEffect(() => {
    if (!open) return;
    /** 在工作区触发器和 Portal 菜单外按下指针时关闭菜单。 */
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !menuRef.current?.contains(target)) setOpen(false);
    };
    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [open]);

  /** 登记服务端目录并切换工作区。 */
  const openDirectory = async (path: string) => {
    const workspace = await api.workspaces.add(path);
    const switched = await switchWithTerminalConfirm(workspace.id, confirm, t);
    if (switched) window.location.reload();
  };

  return (
    <div className="workspace-switcher" ref={rootRef}>
      <button ref={triggerRef} className="workspace-trigger" type="button" onClick={() => setOpen((value) => !value)} aria-expanded={open} title={active?.path}>
        <FolderGit2 size={13} /><strong className={showPath && active ? "path" : undefined}>{showPath && active ? active.path : activeName}</strong><ChevronDown size={12} className={open ? "open" : ""} />
      </button>
      {active?.path && active.path !== activeName && (
        <button
          className="workspace-path-toggle"
          type="button"
          aria-pressed={showPath}
          aria-label={showPath ? t("Show workspace name", "显示工作区名称") : t("Show full path", "显示完整路径")}
          title={showPath ? t("Show workspace name", "显示工作区名称") : t("Show full path", "显示完整路径")}
          onClick={() => setShowPath((value) => !value)}
        >
          <ChevronsLeftRight size={12} />
        </button>
      )}
      {open && createPortal(
        <div ref={menuRef} className="workspace-menu" style={menuStyle}>
          <div className="workspace-menu-head"><span><strong>{activeName}</strong><small>{active?.path}</small></span><button type="button" aria-label={t("Close workspace menu", "关闭工作区菜单")} onClick={() => setOpen(false)}><X size={15} /></button></div>
          <div className="workspace-items">
            {workspaces.data?.workspaces.map((workspace) => (
              <button type="button" className="workspace-item" key={workspace.id} onClick={() => workspace.id !== workspaces.data?.active_id && switchWorkspace.mutate(workspace.id)}>
                <span><strong>{localizeApiMessage(workspace.name, locale)}</strong><small>{workspace.path}</small></span>{workspace.id === workspaces.data?.active_id && <Check size={14} />}
              </button>
            ))}
          </div>
          <button type="button" className="workspace-add" onClick={() => { setOpen(false); setBrowserOpen(true); }}><FolderOpen size={15} /><span>{t("Browse server directories", "浏览服务端目录")}</span></button>
          {switchWorkspace.error && <p className="form-error workspace-error">{switchWorkspace.error.message}</p>}
        </div>,
        document.body
      )}
      <ServerDirectoryDialog open={browserOpen} onClose={() => setBrowserOpen(false)} onSelect={openDirectory} />
    </div>
  );
}
