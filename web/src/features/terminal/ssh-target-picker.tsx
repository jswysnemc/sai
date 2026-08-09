import { useQuery } from "@tanstack/react-query";
import { ChevronDown, Server, SquareTerminal } from "lucide-react";
import { useEffect, useRef, useState } from "react";
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
 * 新建终端时选择本地 Shell 或某台 SSH 主机。
 *
 * 未配置任何 SSH 主机时不展开菜单，直接创建本地终端，
 * 避免为单一选项增加一次点击。
 *
 * @param props 本地与 SSH 创建回调
 * @returns 终端目标选择器
 */
export function SshTargetPicker(props: SshTargetPickerProps) {
  const { t } = useI18n();
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
    document.addEventListener("pointerdown", closeOutside);
    return () => document.removeEventListener("pointerdown", closeOutside);
  }, [open]);

  return (
    <div className="ssh-target-picker" ref={rootRef}>
      <Button
        className="ssh-target-toggle"
        onClick={() => (items.length === 0 ? props.onCreateLocal() : setOpen((value) => !value))}
        aria-label={t("New terminal", "新建终端")}
        aria-expanded={items.length === 0 ? undefined : open}
        title={t("New terminal", "新建终端")}
      >
        <ChevronDown size={12} />
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
        </div>
      )}
    </div>
  );
}
