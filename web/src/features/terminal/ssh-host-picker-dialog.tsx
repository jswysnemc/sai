import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, Server } from "lucide-react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { SshHostForm } from "../settings/ssh/ssh-host-form";
import {
  canSubmitSshHostForm,
  EMPTY_SSH_HOST_FORM,
  sshHostAddress,
  toSshHostInput,
  type SshHostFormState
} from "../settings/ssh/ssh-host-form-state";
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
  const queryClient = useQueryClient();
  const [connectingId, setConnectingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState<SshHostFormState | null>(null);
  const [saving, setSaving] = useState(false);
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

  /**
   * 保存新主机并立即用它建立连接。
   *
   * @returns 无
   */
  const saveDraft = async () => {
    if (!draft || !canSubmitSshHostForm(draft)) return;
    setSaving(true);
    setError(null);
    try {
      const created = await api.ssh.create(toSshHostInput(draft));
      await queryClient.invalidateQueries({ queryKey: ["ssh-hosts"] });
      setDraft(null);
      await pickHost(created.host.id);
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to save the SSH host", "SSH 主机保存失败").message);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      open={open}
      title={draft ? t("Add SSH host", "添加 SSH 主机") : t("New SSH terminal", "新建 SSH 终端")}
      description={draft
        ? t("The host is saved and connected right away", "保存后立即连接")
        : t("Pick a configured host", "选择一台已配置的主机")}
      size="small"
      onClose={onClose}
    >
      <div className="ssh-host-picker">
        {error && <p className="ssh-host-picker-error">{error}</p>}
        {draft ? (
          <>
            <SshHostForm form={draft} onChange={setDraft} />
            <div className="ssh-host-picker-form-actions">
              <Button variant="secondary" disabled={saving} onClick={() => { setDraft(null); setError(null); }}>
                {t("Cancel", "取消")}
              </Button>
              <Button disabled={saving || !canSubmitSshHostForm(draft)} onClick={() => void saveDraft()}>
                {saving ? t("Connecting", "连接中") : t("Save and connect", "保存并连接")}
              </Button>
            </div>
          </>
        ) : (
          <>
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
              <p className="ssh-host-picker-empty">{t("No SSH host configured yet", "尚未配置任何 SSH 主机")}</p>
            )}
            <Button
              variant="secondary"
              className="ssh-host-picker-add"
              disabled={connectingId !== null}
              onClick={() => { setError(null); setDraft(EMPTY_SSH_HOST_FORM); }}
            >
              <Plus size={13} aria-hidden />
              <span>{t("Add SSH host", "添加 SSH 主机")}</span>
            </Button>
          </>
        )}
      </div>
    </Modal>
  );
}
