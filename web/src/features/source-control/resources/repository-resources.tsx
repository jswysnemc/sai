import { useQuery } from "@tanstack/react-query";
import { ArchiveRestore, Play, Plus, RadioTower, Tag, Trash2 } from "lucide-react";
import { useState } from "react";
import { api } from "../../../api/client";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";

type RepositoryResourcesProps = {
  repoRoot: string | null;
  open: boolean;
  busy: boolean;
  runOperation: RunGitOperation;
};

/**
 * 渲染 stash、标签和远端资源，并提供对应管理操作。
 *
 * @param props 菜单状态、忙碌状态和 Git 操作回调
 * @returns 仓库资源菜单分组
 */
export function RepositoryResources(props: RepositoryResourcesProps) {
  const { t } = useI18n();
  const [tagName, setTagName] = useState("");
  const [remoteName, setRemoteName] = useState("");
  const [remoteUrl, setRemoteUrl] = useState("");
  const resources = useQuery({
    queryKey: ["git-resources", props.repoRoot],
    queryFn: () => api.workspace.gitResources(props.repoRoot ?? undefined),
    enabled: props.open,
    staleTime: 5_000
  });

  /**
   * 创建标签并在成功后清空输入。
   *
   * @returns 无返回值
   */
  const createTag = async () => {
    const tag = tagName.trim();
    if (!tag) return;
    const result = await props.runOperation("tag_create", { tag });
    if (result?.ok) setTagName("");
  };

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

  if (resources.isLoading) return <div className="git-resource-state">{t("Loading repository resources...", "正在读取仓库资源…")}</div>;
  if (resources.error) return <div className="git-resource-state error">{resources.error.message}</div>;

  return (
    <>
      <span>{t("Stashes", "储藏记录")}</span>
      {(resources.data?.stashes ?? []).slice(0, 8).map((stash) => (
        <div className="git-resource-row" key={stash.reference}>
          <span title={stash.subject}><strong>{stash.reference}</strong><small>{stash.subject}</small></span>
          <div>
            <Button disabled={props.busy} title={t("Apply stash", "应用储藏")} onClick={() => void props.runOperation("stash_apply", { stash_ref: stash.reference })}><Play size={11} /></Button>
            <Button disabled={props.busy} title={t("Pop stash", "弹出储藏")} onClick={() => void props.runOperation("stash_pop", { stash_ref: stash.reference })}><ArchiveRestore size={11} /></Button>
            <Button disabled={props.busy} title={t("Drop stash", "删除储藏")} onClick={() => void props.runOperation("stash_drop", {
              stash_ref: stash.reference,
              confirmTitle: t("Drop stash?", "删除储藏记录？"),
              confirmDescription: stash.subject
            })}><Trash2 size={11} /></Button>
          </div>
        </div>
      ))}
      {(resources.data?.stashes.length ?? 0) === 0 && <div className="git-resource-state">{t("No stashes", "没有储藏记录")}</div>}

      <span>{t("Tags", "标签")}</span>
      <div className="git-resource-create compact">
        <Tag size={12} />
        <input value={tagName} onChange={(event) => setTagName(event.target.value)} placeholder={t("Tag name", "标签名称")} spellCheck={false} />
        <Button disabled={props.busy || !tagName.trim()} onClick={() => void createTag()} title={t("Create tag at HEAD", "在 HEAD 创建标签")}><Plus size={11} /></Button>
      </div>
      {(resources.data?.tags ?? []).slice(0, 8).map((tag) => (
        <div className="git-resource-row" key={tag.name}>
          <span title={tag.subject}><strong>{tag.name}</strong><small>{tag.sha.slice(0, 7)}</small></span>
          <div>
            <Button disabled={props.busy} title={t("Delete tag", "删除标签")} onClick={() => void props.runOperation("tag_delete", {
              tag: tag.name,
              confirmTitle: t("Delete tag?", "删除标签？"),
              confirmDescription: tag.name
            })}><Trash2 size={11} /></Button>
          </div>
        </div>
      ))}

      <span>{t("Remotes", "远端")}</span>
      <div className="git-resource-create">
        <RadioTower size={12} />
        <input value={remoteName} onChange={(event) => setRemoteName(event.target.value)} placeholder={t("Name", "名称")} spellCheck={false} />
        <input value={remoteUrl} onChange={(event) => setRemoteUrl(event.target.value)} placeholder={t("Remote URL", "远端地址")} spellCheck={false} />
        <Button disabled={props.busy || !remoteName.trim() || !remoteUrl.trim()} onClick={() => void addRemote()} title={t("Add remote", "新增远端")}><Plus size={11} /></Button>
      </div>
      {(resources.data?.remotes ?? []).map((remote) => (
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
