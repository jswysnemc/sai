import { ArchiveRestore, Eye, Play, Trash2 } from "lucide-react";
import { useState } from "react";
import type { GitStashEntry } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";
import { StashPreviewDialog } from "./stash-preview-dialog";

type StashSectionProps = {
  stashes: GitStashEntry[];
  repoRoot: string | null;
  busy: boolean;
  runOperation: RunGitOperation;
};

/**
 * 渲染 stash 列表及预览、应用、弹出和删除操作。
 *
 * @param props stash 数据、仓库和 Git 操作回调
 * @returns stash 资源分区
 */
export function StashSection(props: StashSectionProps) {
  const { t } = useI18n();
  const [preview, setPreview] = useState<GitStashEntry | null>(null);

  return (
    <>
      <span>{t("Stashes", "储藏记录")}</span>
      {props.stashes.slice(0, 8).map((stash) => (
        <div className="git-resource-row" key={stash.reference}>
          <span title={stash.subject}><strong>{stash.reference}</strong><small>{stash.subject}</small></span>
          <div>
            <Button title={t("Show stash", "查看储藏内容")} onClick={() => setPreview(stash)}><Eye size={11} /></Button>
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
      {props.stashes.length === 0 && <div className="git-resource-state">{t("No stashes", "没有储藏记录")}</div>}
      <StashPreviewDialog
        stash={preview}
        repoRoot={props.repoRoot}
        onClose={() => setPreview(null)}
      />
    </>
  );
}
