import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Server, Settings2 } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
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
  /** 选定主机后创建会话；失败时抛错交由对话框展示 */
  onPick: (hostId: string) => Promise<void> | void;
};

/**
 * 选择 SSH 主机后新建远程终端。
 *
 * 未配置主机时给出前往设置的入口而非空列表：
 * 空列表只说明"没有",不说明"怎么办"。
 *
 * 连接失败时留在对话框内展示原因：先关窗再异步失败的话，
 * 用户看到的只是什么都没发生。
 *
 * @param props 展开状态与选择回调
 * @returns 主机选择对话框
 */
export function SshHostPickerDialog({ open, onClose, onPick }: SshHostPickerDialogProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [connectingId, setConnectingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const hosts = useQuery({
    queryKey: ["ssh-hosts"],
    queryFn: api.ssh.list,
    staleTime: 30_000,
    enabled: open
  });
  const items = hosts.data?.hosts ?? [];

  /**
   * 连接选中的主机，成功后关闭对话框。
   *
   * @param hostId 目标主机标识
   * @returns 无
   */
  const pickHost = async (hostId: string) => {
    setConnectingId(hostId);
    setError(null);
    try {
      await onPick(hostId);
      onClose();
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to open the SSH terminal", "SSH 终端创建失败").message);
    } finally {
      setConnectingId(null);
    }
  };

  return (
    <Modal
      open={open}
      title={t("New SSH terminal", "新建 SSH 终端")}
      description={t("Pick a configured host", "选择一台已配置的主机")}
      size="small"
      onClose={onClose}
    >
      <div className="ssh-host-picker">
        {error && <p className="ssh-host-picker-error">{error}</p>}
        {items.map((host) => (
          <button
            type="button"
            key={host.id}
            className="ssh-host-picker-item"
            disabled={connectingId !== null}
            onClick={() => void pickHost(host.id)}
          >
            <Server size={14} aria-hidden />
            <span>{host.label}</span>
            <small>{connectingId === host.id ? t("Connecting", "连接中") : sshHostAddress(host)}</small>
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
