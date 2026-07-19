import { Download, Folder } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { useI18n } from "../../i18n/use-i18n";
import "./source-control-empty-state.css";

export type CloneRepositoryInput = {
  remoteUrl: string;
  directory?: string;
};

type CloneRepositoryDialogProps = {
  open: boolean;
  onClose: () => void;
  onContinue: (input: CloneRepositoryInput) => void;
};

/**
 * 收集 Git 克隆地址和可选目标目录名。
 *
 * @param props 打开状态、关闭回调与下一步回调
 * @returns Git 克隆输入弹层
 */
export function CloneRepositoryDialog(props: CloneRepositoryDialogProps) {
  const { t } = useI18n();
  const [remoteUrl, setRemoteUrl] = useState("");
  const [directory, setDirectory] = useState("");

  useEffect(() => {
    if (!props.open) return;
    setRemoteUrl("");
    setDirectory("");
  }, [props.open]);

  /**
   * 提交克隆参数并进入服务端目标目录选择。
   *
   * @returns 无返回值
   */
  const submit = () => {
    const url = remoteUrl.trim();
    if (!url) return;
    props.onContinue({
      remoteUrl: url,
      directory: directory.trim() || undefined
    });
  };

  return (
    <Modal
      open={props.open}
      title={t("Clone Repository", "克隆仓库")}
      description={t("Enter a Git HTTPS, SSH, or local repository address. The next step selects the destination parent directory.", "输入 Git HTTPS、SSH 或本地仓库地址，下一步选择目标父目录。")}
      size="small"
      onClose={props.onClose}
      footer={(
        <>
          <Button onClick={props.onClose}>{t("Cancel", "取消")}</Button>
          <Button variant="primary" disabled={!remoteUrl.trim()} onClick={submit}>
            <Folder size={14} />
            {t("Choose Destination", "选择目标目录")}
          </Button>
        </>
      )}
    >
      <div className="clone-repository-form">
        <label>
          <span><Download size={13} />{t("Repository URL", "仓库地址")}</span>
          <input
            autoFocus
            value={remoteUrl}
            placeholder="git@github.com:owner/repository.git"
            spellCheck={false}
            onChange={(event) => setRemoteUrl(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && remoteUrl.trim()) submit();
            }}
          />
        </label>
        <label>
          <span><Folder size={13} />{t("Folder name", "文件夹名称")}</span>
          <input
            value={directory}
            placeholder={t("Derived from repository URL", "根据仓库地址自动推导")}
            spellCheck={false}
            onChange={(event) => setDirectory(event.target.value)}
          />
          <small>{t("Optional. Only a single folder name is accepted.", "可选，仅接受单层文件夹名称。")}</small>
        </label>
      </div>
    </Modal>
  );
}
