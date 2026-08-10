import { useQuery } from "@tanstack/react-query";
import { Server, Settings2 } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { api } from "../../api/client";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { sshHostAddress } from "../settings/ssh/ssh-host-form-state";
import { useI18n } from "../i18n/use-i18n";
import "./ssh-host-picker-dialog.css";

type SshHostPickerDialogProps = {
  /** 是否展开 */
  open: boolean;
  /** 关闭对话框 */
  onClose: () => void;
  /** 选定主机后创建会话 */
  onPick: (hostId: string) => void;
};

/**
 * 选择 SSH 主机后新建远程终端。
 *
 * 未配置主机时给出前往设置的入口而非空列表：
 * 空列表只说明"没有",不说明"怎么办"。
 *
 * @param props 展开状态与选择回调
 * @returns 主机选择对话框
 */
export function SshHostPickerDialog({ open, onClose, onPick }: SshHostPickerDialogProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const hosts = useQuery({
    queryKey: ["ssh-hosts"],
    queryFn: api.ssh.list,
    staleTime: 30_000,
    enabled: open
  });
  const items = hosts.data?.hosts ?? [];

  return (
    <Modal
      open={open}
      title={t("New SSH terminal", "新建 SSH 终端")}
      description={t("Pick a configured host", "选择一台已配置的主机")}
      size="small"
      onClose={onClose}
    >
      <div className="ssh-host-picker">
        {items.map((host) => (
          <button
            type="button"
            key={host.id}
            className="ssh-host-picker-item"
            onClick={() => {
              onClose();
              onPick(host.id);
            }}
          >
            <Server size={14} aria-hidden />
            <span>{host.label}</span>
            <small>{sshHostAddress(host)}</small>
          </button>
        ))}
        {items.length === 0 && (
          <div className="ssh-host-picker-empty">
            <p>{t("No SSH host configured yet", "尚未配置任何 SSH 主机")}</p>
            <Button
              variant="secondary"
              onClick={() => {
                onClose();
                navigate("/settings/ssh");
              }}
            >
              <Settings2 size={13} aria-hidden />
              <span>{t("Add SSH host", "添加 SSH 主机")}</span>
            </Button>
          </div>
        )}
      </div>
    </Modal>
  );
}
