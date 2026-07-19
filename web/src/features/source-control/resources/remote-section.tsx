import { Plus, RadioTower, Trash2 } from "lucide-react";
import { useState } from "react";
import type { GitRemote } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";

type RemoteSectionProps = {
  remotes: GitRemote[];
  busy: boolean;
  runOperation: RunGitOperation;
};

/**
 * 渲染远端创建、列表和删除操作。
 *
 * @param props 远端数据、忙碌状态和 Git 操作回调
 * @returns 远端资源分区
 */
export function RemoteSection(props: RemoteSectionProps) {
  const { t } = useI18n();
  const [remoteName, setRemoteName] = useState("");
  const [remoteUrl, setRemoteUrl] = useState("");

  /**
   * 新增远端并在成功后清空输入。
   *
   * @returns 无返回值
   */
  const addRemote = async () => {
    const name = remoteName.trim();
    const url = remoteUrl.trim();
    if (!name || !url) return;
    const result = await props.runOperation("remote_add", { remote_name: name, remote_url: url });
    if (!result?.ok) return;
    setRemoteName("");
    setRemoteUrl("");
  };

  return (
    <>
      <span>{t("Remotes", "远端")}</span>
      <div className="git-resource-create">
        <RadioTower size={12} />
        <input value={remoteName} onChange={(event) => setRemoteName(event.target.value)} placeholder={t("Name", "名称")} spellCheck={false} />
        <input value={remoteUrl} onChange={(event) => setRemoteUrl(event.target.value)} placeholder={t("Remote URL", "远端地址")} spellCheck={false} />
        <Button disabled={props.busy || !remoteName.trim() || !remoteUrl.trim()} onClick={() => void addRemote()} title={t("Add remote", "新增远端")}><Plus size={11} /></Button>
      </div>
      {props.remotes.map((remote) => (
        <div className="git-resource-row" key={remote.name}>
          <span title={remote.fetch_url || remote.push_url}><strong>{remote.name}</strong><small>{remote.fetch_url || remote.push_url}</small></span>
          <div>
            <Button disabled={props.busy} title={t("Remove remote", "删除远端")} onClick={() => void props.runOperation("remote_remove", {
              remote_name: remote.name,
              confirmTitle: t("Remove remote?", "删除远端？"),
              confirmDescription: `${remote.name} · ${remote.fetch_url || remote.push_url}`
            })}><Trash2 size={11} /></Button>
          </div>
        </div>
      ))}
    </>
  );
}
