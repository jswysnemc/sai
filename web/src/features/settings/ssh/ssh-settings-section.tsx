import { Download, Pencil, Plus, Terminal, Trash2 } from "../../../shared/ui/icons";
import { useEffect, useMemo, useState } from "react";
import { api } from "../../../api/client";
import type { SshHost } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { Modal } from "../../../shared/ui/dialog/modal";
import { useI18n } from "../../i18n/use-i18n";
import { DataTable, SettingsPanel, SkTextInput } from "../kit";
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
import { useSshTerminal } from "./use-ssh-terminal";
import { SshTerminalDialog } from "./ssh-terminal-dialog";
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
  const connection = useSshTerminal();
  const [query, setQuery] = useState("");
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

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return hosts.filter((host) => [host.label, sshHostAddress(host)].some((value) => value.toLowerCase().includes(needle)));
  }, [hosts, query]);
  return <>
    <SettingsPanel title={t("Hosts", "主机")} description={t("The Sai server establishes connections. Passwords are requested when connecting; only private key paths are stored.", "连接由 Sai 服务端发起。密码在连接时输入，此处只保存私钥路径。")} actions={<>
      <Button variant="primary" onClick={startCreate}><Plus size={14} />{t("Add host", "新增主机")}</Button>
      <Button variant="secondary" onClick={() => setImportOpen(true)}><Download size={14} />{t("Import from ~/.ssh/config", "从 ~/.ssh/config 导入")}</Button>
    </>}>
      <SkTextInput type="search" value={query} onChange={setQuery} aria-label={t("Search hosts", "搜索主机")} placeholder={t("Search name or address", "搜索名称或地址")} />
      <DataTable label={t("SSH hosts", "SSH 主机")} rows={visible} rowKey={(host) => host.id} columns={[
        { id: "name", header: t("Name", "名称"), sortValue: (host) => host.label, render: (host) => host.label },
        { id: "address", header: t("Address", "地址"), sortValue: sshHostAddress, render: (host) => <span className="break-all">{sshHostAddress(host)}</span> },
        { id: "actions", header: t("Actions", "操作"), render: (host) => <div className="flex gap-1">
          <Button variant="ghost" size="icon" disabled={connection.busy} onClick={() => void connection.open(host)} aria-label={t(`Open ${host.label} in a terminal`, `在终端中打开 ${host.label}`)} title={t("Open in terminal", "在终端中打开")}><Terminal size={14} /></Button>
          <Button variant="ghost" size="icon" onClick={() => startEdit(host)} aria-label={t("Edit host", "编辑主机")}><Pencil size={14} /></Button>
          <Button variant="ghost-danger" size="icon" onClick={() => void remove(host)} aria-label={t("Remove host", "删除主机")}><Trash2 size={14} /></Button>
        </div> }
      ]} />
      {error && <p className="ssh-host-error">{error}</p>}
    </SettingsPanel>
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
      <SshTerminalDialog connection={connection} />
    </>;
}
