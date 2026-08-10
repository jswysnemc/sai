import { useQuery } from "@tanstack/react-query";
import { Plus, Server, Settings2, SquareTerminal } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api } from "../../api/client";
import { Button } from "../../shared/ui/button/button";
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
 * @param props 本地与 SSH 创建回调
 * @returns 新建终端按钮
 */
export function SshTargetPicker(props: SshTargetPickerProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const hosts = useQuery({ queryKey: ["ssh-hosts"], queryFn: api.ssh.list, staleTime: 30_000 });
  const items = hosts.data?.hosts ?? [];

  useEffect(() => {
    if (!open) return;
    /** 点击菜单外部时收起。 */
    const closeOutside = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
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
        onClick={() => setOpen((value) => !value)}
        aria-label={t("New terminal", "新建终端")}
        aria-expanded={open}
        aria-haspopup="menu"
        title={t("New terminal", "新建终端")}
      >
        <Plus size={14} />
      </Button>
      {open && (
        <div className="ssh-target-menu" role="menu">
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
        </div>
      )}
    </div>
  );
}
