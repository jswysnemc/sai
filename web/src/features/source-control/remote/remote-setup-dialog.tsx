import { RefreshCw, Upload } from "lucide-react";
import { useRef } from "react";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { useI18n } from "../../i18n/use-i18n";
import type { RemoteDependentAction } from "./remote-setup-trigger";
import "./remote-setup-dialog.css";

type RemoteSetupDialogProps = {
  open: boolean;
  /** 因缺少远端而失败的操作，决定文案与提交按钮 */
  action: RemoteDependentAction;
  /** 当前仓库工作目录，供用户确认操作对象 */
  workdir: string;
  /** 当前分支名 */
  branch: string;
  remoteUrl: string;
  busy: boolean;
  error: string;
  onRemoteUrlChange: (value: string) => void;
  onClose: () => void;
  onSubmit: () => void;
};

/**
 * 引导用户在远端缺失时补配 origin，并接着重试原操作。
 *
 * 拉取、推送等操作在未配置远端时失败，此弹层承接该失败：
 * 保存远端地址后自动重试触发它的那次操作，省去用户手工再点一次。
 *
 * @param props 弹层状态、目标操作与远端地址回调
 * @returns 远端配置引导弹层
 */
export function RemoteSetupDialog(props: RemoteSetupDialogProps) {
  const { t } = useI18n();
  const inputRef = useRef<HTMLInputElement>(null);

  // 1. 按触发操作切换说明文案，让用户知道补配远端后会发生什么
  const descriptions: Record<RemoteDependentAction, string> = {
    fetch: t(
      "This repository has no remote configured, so changes cannot be fetched.",
      "此仓库尚未配置远端，无法获取远端改动。"
    ),
    pull: t(
      "This repository has no remote configured, so changes cannot be pulled.",
      "此仓库尚未配置远端，无法拉取远端改动。"
    ),
    pull_rebase: t(
      "This repository has no remote configured, so changes cannot be pulled.",
      "此仓库尚未配置远端，无法拉取远端改动。"
    ),
    push: t(
      "This repository has no remote configured, so commits cannot be pushed.",
      "此仓库尚未配置远端，无法推送提交。"
    ),
    sync: t(
      "This repository has no remote configured, so it cannot be synchronized.",
      "此仓库尚未配置远端，无法与远端同步。"
    )
  };

  // 2. 提交按钮直接说明保存后要继续做的事
  const submitLabels: Record<RemoteDependentAction, string> = {
    fetch: t("Save and fetch", "保存并获取"),
    pull: t("Save and pull", "保存并拉取"),
    pull_rebase: t("Save and pull", "保存并拉取"),
    push: t("Save and push", "保存并推送"),
    sync: t("Save and sync", "保存并同步")
  };

  const submittable = Boolean(props.remoteUrl.trim()) && !props.busy;

  return (
    <Modal
      open={props.open}
      title={t("Set up a remote", "配置远端仓库")}
      description={descriptions[props.action]}
      size="small"
      initialFocusRef={inputRef}
      onClose={props.onClose}
      footer={
        <>
          <Button variant="secondary" onClick={props.onClose} disabled={props.busy}>
            {t("Cancel", "取消")}
          </Button>
          <Button
            variant="primary"
            onClick={props.onSubmit}
            disabled={!submittable}
            aria-busy={props.busy || undefined}
          >
            {props.action === "push" ? <Upload size={13} /> : <RefreshCw size={13} />}
            {submitLabels[props.action]}
          </Button>
        </>
      }
    >
      <form
        className="git-remote-setup"
        onSubmit={(event) => {
          event.preventDefault();
          if (submittable) props.onSubmit();
        }}
      >
        <div className="git-remote-setup-meta">
          <span title={props.branch}>{props.branch}</span>
          <span title={props.workdir}>{props.workdir}</span>
        </div>
        <label className="git-remote-setup-field">
          <span>{t("Remote URL", "远端地址")}</span>
          <input
            ref={inputRef}
            value={props.remoteUrl}
            onChange={(event) => props.onRemoteUrlChange(event.target.value)}
            placeholder="git@github.com:owner/repository.git"
            spellCheck={false}
            disabled={props.busy}
          />
        </label>
        {props.error && <p className="git-remote-setup-error">{props.error}</p>}
      </form>
    </Modal>
  );
}
