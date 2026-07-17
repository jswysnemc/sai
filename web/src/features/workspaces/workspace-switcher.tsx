import { useMutation, useQuery } from "@tanstack/react-query";
import { Check, ChevronDown, FolderGit2, FolderOpen, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "../../api/client";
import { localizeApiMessage } from "../../api/api-error";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { useAnchoredPopover } from "../../shared/ui/popover/use-anchored-popover";
import { ServerDirectoryDialog } from "./server-directory-dialog";
import "./workspace-switcher.css";
import { useI18n } from "../i18n/use-i18n";
import type { Translate } from "../i18n/i18n-context";

/**
 * 渲染紧凑工作区入口、最近工作区和服务端目录浏览器。
 *
 * @returns 工作区选择器
 */
export function WorkspaceSwitcher() {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(false);
  const [browserOpen, setBrowserOpen] = useState(false);
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
      <button ref={triggerRef} className="workspace-trigger" type="button" onClick={() => setOpen((value) => !value)} aria-expanded={open}>
        <FolderGit2 size={13} /><strong>{activeName}</strong><ChevronDown size={12} className={open ? "open" : ""} />
      </button>
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

/**
 * 切换工作区，遇到终端占用冲突时经确认后关闭终端重试。
 *
 * @param id 目标工作区 ID
 * @param confirm 全局确认对话框方法
 * @param t 双语文本选择方法
 * @returns 是否完成切换
 */
export async function switchWithTerminalConfirm(
  id: string,
  confirm: (options: { title: string; description: string; confirmLabel?: string; danger?: boolean }) => Promise<boolean>,
  t: Translate
): Promise<boolean> {
  try {
    // 1. 先尝试普通切换
    await api.workspaces.switch(id);
    return true;
  } catch (error) {
    // 2. 非终端占用错误直接抛出
    const message = error instanceof Error ? error.message : String(error);
    if (!message.includes("terminal")) throw error;
    // 3. 询问用户是否关闭全部终端并切换
    const confirmed = await confirm({
      title: t("Close terminals and switch workspace", "关闭终端并切换工作区"),
      description: t("Terminal sessions are running. Close all terminals and switch workspace?", "当前有终端会话在运行，关闭全部终端并切换？"),
      confirmLabel: t("Close and switch", "关闭并切换"),
      danger: true
    });
    if (!confirmed) return false;
    // 4. 携带 close_terminals=true 重试
    await api.workspaces.switch(id, true);
    return true;
  }
}
