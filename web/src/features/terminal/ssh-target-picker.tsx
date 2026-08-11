import { useQuery } from "@tanstack/react-query";
import { Plus, Server, Settings2, SquareTerminal } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import { api } from "../../api/client";
import { Button } from "../../shared/ui/button/button";
import { useAnchoredPopover } from "../../shared/ui/popover/use-anchored-popover";
import { sshHostAddress } from "../settings/ssh/ssh-host-form-state";
import { useI18n } from "../i18n/use-i18n";
import "./ssh-target-picker.css";

type SshTargetPickerProps = {
  onCreateLocal: () => void;
  onCreateSsh: (hostId: string) => void;
};

/**
 * 新建终端按钮，可选择本地 Shell 或某台 SSH 主机。
 *
 * 菜单始终展开，未配置主机时给出前往 SSH 设置的入口：
 * 直接创建本地终端会让 SSH 能力完全不可见，用户无从得知它存在。
 *
 * 菜单渲染到 Portal：触发器位于标签栏的横向滚动容器内，
 * 就地绝对定位会被 overflow 裁剪，导致菜单看不见。
 *
 * @param props 本地与 SSH 创建回调
 * @returns 新建终端按钮
 */
export function SshTargetPicker(props: SshTargetPickerProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const hosts = useQuery({ queryKey: ["ssh-hosts"], queryFn: api.ssh.list, staleTime: 30_000 });
  const items = hosts.data?.hosts ?? [];
  const menuStyle = useAnchoredPopover({ open, anchorRef: rootRef, preferredWidth: 208, minimumWidth: 208, align: "right", maxHeight: 256 });

  useEffect(() => {
    if (!open) return;
    /** 在触发器和 Portal 菜单外按下指针时收起。 */
    const closeOutside = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !menuRef.current?.contains(target)) setOpen(false);
    };
    /** 按下 Esc 时收起，保持与其他浮层一致。 */
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", closeOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [open]);

  return (
    <div className="ssh-target-picker" ref={rootRef}>
      <Button
        className="bottom-terminal-new"
        onClick={(event) => {
          // #region agent log
          const rect = rootRef.current?.getBoundingClientRect();
          const under = document.elementFromPoint(event.clientX, event.clientY);
          fetch('http://127.0.0.1:7716/ingest/0150b615-e4f4-4cb9-b2bc-b348cdf7556f',{method:'POST',headers:{'Content-Type':'application/json','X-Debug-Session-Id':'dcb5f5'},body:JSON.stringify({sessionId:'dcb5f5',runId:'pre-fix',hypothesisId:'C',location:'ssh-target-picker.tsx:onClick',message:'plus button click fired',data:{openBefore:!open,clientX:event.clientX,clientY:event.clientY,underTag:under?.nodeName,underClass:(under as Element|null)?.className??null,anchorRect:rect?{top:rect.top,left:rect.left,width:rect.width,height:rect.height}:null},timestamp:Date.now()})}).catch(()=>{});
          // #endregion
          setOpen((value) => !value);
        }}
        aria-label={t("New terminal", "新建终端")}
        aria-expanded={open}
        aria-haspopup="menu"
        title={t("New terminal", "新建终端")}
      >
        <Plus size={14} />
      </Button>
      {open && createPortal(
        <div ref={menuRef} className="ssh-target-menu" role="menu" style={menuStyle}>
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              props.onCreateLocal();
            }}
          >
            <SquareTerminal size={13} />
            <span>{t("Local shell", "本地 Shell")}</span>
          </button>
          <div className="ssh-target-separator" />
          {items.map((host) => (
            <button
              type="button"
              role="menuitem"
              key={host.id}
              onClick={() => {
                setOpen(false);
                props.onCreateSsh(host.id);
              }}
            >
              <Server size={13} />
              <span>{host.label}</span>
              <small>{sshHostAddress(host)}</small>
            </button>
          ))}
          {items.length === 0 && (
            <button
              type="button"
              role="menuitem"
              className="ssh-target-configure"
              onClick={() => {
                setOpen(false);
                navigate("/settings/ssh");
              }}
            >
              <Settings2 size={13} />
              <span>{t("Add SSH host", "添加 SSH 主机")}</span>
            </button>
          )}
        </div>,
        document.body
      )}
    </div>
  );
}
