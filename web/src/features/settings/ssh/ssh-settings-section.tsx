import { Download, Pencil, Plus, Server, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../../api/client";
import type { SshHost } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { Modal } from "../../../shared/ui/dialog/modal";
import { useI18n } from "../../i18n/use-i18n";
import { EditorHeader, SettingsGroup } from "../editor-layout";
import { SshHostForm } from "./ssh-host-form";
import {
  EMPTY_SSH_HOST_FORM,
  canSubmitSshHostForm,
  sshHostAddress,
  toSshHostForm,
  toSshHostInput,
  type SshHostFormState
} from "./ssh-host-form-state";
import { SshImportDialog } from "./ssh-import-dialog";
import "./ssh-settings.css";

/**
 * 渲染 SSH 主机管理设置。
 *
 * 主机列表独立于应用配置面板的补丁式更新：它有自己的增删改接口，
 * 每次操作直接落库并重新拉取，避免与其他设置项的保存时机耦合。
 *
 * @returns SSH 设置区
 */
export function SshSettingsSection() {
  const { t } = useI18n();
  const confirm = useConfirm();
  const [hosts, setHosts] = useState<SshHost[]>([]);
  const [editing, setEditing] = useState<SshHost | null>(null);
  const [form, setForm] = useState<SshHostFormState>(EMPTY_SSH_HOST_FORM);
  const [formOpen, setFormOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  /**
   * 重新拉取主机列表。
   *
   * @returns 无
   */
  const refresh = async () => {
    try {
      const result = await api.ssh.list();
      setHosts(result.hosts);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  /**
   * 打开新增主机表单。
   *
   * @returns 无
   */
  const startCreate = () => {
    setEditing(null);
    setForm(EMPTY_SSH_HOST_FORM);
    setError("");
    setFormOpen(true);
  };

  /**
   * 打开指定主机的编辑表单。
   *
   * @param host 待编辑主机
   * @returns 无
   */
  const startEdit = (host: SshHost) => {
    setEditing(host);
    setForm(toSshHostForm(host));
    setError("");
    setFormOpen(true);
  };

  /**
   * 保存新增或编辑结果。
   *
   * @returns 无
   */
  const save = async () => {
    setBusy(true);
    setError("");
    try {
      const input = toSshHostInput(form);
      if (editing) await api.ssh.update(editing.id, input);
      else await api.ssh.create(input);
      setFormOpen(false);
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  };

  /**
   * 删除指定主机。
   *
   * @param host 待删除主机
   * @returns 无
   */
  const remove = async (host: SshHost) => {
    const confirmed = await confirm({
      title: t("Remove host", "删除主机"),
      description: t(`Remove ${host.label} from the SSH host list?`, `确定从 SSH 主机列表中删除 ${host.label}？`),
      confirmLabel: t("Remove", "删除"),
      danger: true
    });
    if (!confirmed) return;
    try {
      await api.ssh.remove(host.id);
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  return (
    <>
      <EditorHeader
        kicker={t("Remote access", "远程访问")}
        title={t("SSH", "SSH")}
        description={t(
          "Hosts for the remote terminal and the agent's SSH tools. Only the private key path is stored — passwords are typed when connecting. Enable the SSH group on the Agent page so the model can use these hosts.",
          "这些主机同时给远程终端和 Agent 的 SSH 工具用。只保存私钥路径，密码在连接时输入。要让模型用这些主机，请在 Agent 配置里打开「SSH 远程」工具组。"
        )}
      />

      <SettingsGroup
        title={t("Hosts", "主机")}
        description={t(
          "Sai connects from the machine running the server, not from the browser.",
          "连接由运行 Sai 服务的机器发起，而非浏览器所在机器。"
        )}
      >
        <div className="ssh-host-actions">
          <Button variant="primary" onClick={startCreate}>
            <Plus size={13} />
            {t("Add host", "新增主机")}
          </Button>
          <Button variant="secondary" onClick={() => setImportOpen(true)}>
            <Download size={13} />
            {t("Import from ~/.ssh/config", "从 ~/.ssh/config 导入")}
          </Button>
        </div>

        {hosts.length === 0 ? (
          <p className="ssh-host-empty">{t("No SSH hosts configured yet.", "尚未配置 SSH 主机。")}</p>
        ) : (
          <ul className="ssh-host-list">
            {hosts.map((host) => (
              <li key={host.id}>
                <Server size={14} />
                <div className="ssh-host-info">
                  <strong>{host.label}</strong>
                  <span>{sshHostAddress(host)}</span>
                </div>
                <button type="button" onClick={() => startEdit(host)} aria-label={t("Edit host", "编辑主机")}>
                  <Pencil size={13} />
                </button>
                <button
                  type="button"
                  className="ssh-host-remove"
                  onClick={() => void remove(host)}
                  aria-label={t("Remove host", "删除主机")}
                >
                  <Trash2 size={13} />
                </button>
              </li>
            ))}
          </ul>
        )}
        {error && <p className="ssh-host-error">{error}</p>}
      </SettingsGroup>

      <Modal
        open={formOpen}
        title={editing ? t("Edit host", "编辑主机") : t("Add host", "新增主机")}
        onClose={() => setFormOpen(false)}
        footer={
          <>
            <Button variant="secondary" onClick={() => setFormOpen(false)} disabled={busy}>
              {t("Cancel", "取消")}
            </Button>
            <Button variant="primary" onClick={() => void save()} disabled={busy || !canSubmitSshHostForm(form)}>
              {t("Save", "保存")}
            </Button>
          </>
        }
      >
        <SshHostForm form={form} onChange={setForm} />
        {error && <p className="ssh-host-error">{error}</p>}
      </Modal>

      <SshImportDialog open={importOpen} onClose={() => setImportOpen(false)} onImported={() => void refresh()} />
    </>
  );
}
