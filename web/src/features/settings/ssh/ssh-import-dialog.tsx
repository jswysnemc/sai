import { Download, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../../api/client";
import type { SshImportCandidate } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { useI18n } from "../../i18n/use-i18n";
import { sshHostAddress } from "./ssh-host-form-state";

type SshImportDialogProps = {
  open: boolean;
  onClose: () => void;
  onImported: () => void;
};

/**
 * 从 `~/.ssh/config` 挑选主机批量导入。
 *
 * 已存在同地址的候选默认不勾选，避免重复导入产生并列条目。
 *
 * @param props 弹层开关与导入完成回调
 * @returns 主机导入弹层
 */
export function SshImportDialog(props: SshImportDialogProps) {
  const { t } = useI18n();
  const [candidates, setCandidates] = useState<SshImportCandidate[]>([]);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!props.open) return;
    let cancelled = false;
    setLoading(true);
    setError("");
    api.ssh
      .scan()
      .then((result) => {
        if (cancelled) return;
        setCandidates(result.candidates);
        // 默认勾选尚未存在的主机
        setSelected(
          new Set(result.candidates.map((candidate, index) => (candidate.duplicate ? -1 : index)).filter((index) => index >= 0))
        );
      })
      .catch((reason: unknown) => {
        if (!cancelled) setError(reason instanceof Error ? reason.message : String(reason));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [props.open]);

  /**
   * 切换单个候选主机的勾选状态。
   *
   * @param index 候选序号
   * @returns 无
   */
  const toggle = (index: number) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  };

  /**
   * 导入选中的主机。
   *
   * @returns 无
   */
  const submit = async () => {
    setImporting(true);
    setError("");
    try {
      const hosts = candidates
        .filter((_, index) => selected.has(index))
        .map((candidate) => ({
          label: candidate.label,
          hostname: candidate.hostname,
          port: candidate.port,
          username: candidate.username,
          identity_file: candidate.identity_file,
          remote_directory: ""
        }));
      await api.ssh.import(hosts);
      props.onImported();
      props.onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setImporting(false);
    }
  };

  return (
    <Modal
      open={props.open}
      title={t("Import from ~/.ssh/config", "从 ~/.ssh/config 导入")}
      description={t("Select the hosts to add to Sai.", "选择要加入 Sai 的主机。")}
      onClose={props.onClose}
      footer={
        <>
          <Button variant="secondary" onClick={props.onClose} disabled={importing}>
            {t("Cancel", "取消")}
          </Button>
          <Button variant="primary" onClick={() => void submit()} disabled={importing || selected.size === 0}>
            {importing ? <Loader2 size={13} className="ssh-spin" /> : <Download size={13} />}
            {t(`Import ${selected.size}`, `导入 ${selected.size} 个`)}
          </Button>
        </>
      }
    >
      {loading ? (
        <p className="ssh-import-empty">{t("Reading ~/.ssh/config...", "正在读取 ~/.ssh/config…")}</p>
      ) : candidates.length === 0 ? (
        <p className="ssh-import-empty">{t("No importable hosts were found.", "没有找到可导入的主机。")}</p>
      ) : (
        <ul className="ssh-import-list">
          {candidates.map((candidate, index) => (
            <li key={`${candidate.hostname}:${candidate.port}:${index}`}>
              <label>
                <input type="checkbox" checked={selected.has(index)} onChange={() => toggle(index)} />
                <span className="ssh-import-label">{candidate.label}</span>
                <span className="ssh-import-address">{sshHostAddress(candidate)}</span>
                {candidate.duplicate && <span className="ssh-import-duplicate">{t("Already added", "已存在")}</span>}
              </label>
            </li>
          ))}
        </ul>
      )}
      {error && <p className="ssh-import-error">{error}</p>}
    </Modal>
  );
}
