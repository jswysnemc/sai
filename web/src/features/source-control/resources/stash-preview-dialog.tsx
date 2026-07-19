import { useQuery } from "@tanstack/react-query";
import type { GitStashEntry } from "../../../api/contracts";
import { api } from "../../../api/client";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { DiffView } from "../../chat/tool-renderers/diff-view";
import { useI18n } from "../../i18n/use-i18n";

type StashPreviewDialogProps = {
  stash: GitStashEntry | null;
  repoRoot: string | null;
  onClose: () => void;
};

/**
 * 在统一弹层中读取并展示 stash Diff。
 *
 * @param props stash、仓库和关闭回调
 * @returns stash 预览弹层
 */
export function StashPreviewDialog(props: StashPreviewDialogProps) {
  const { t } = useI18n();
  const preview = useQuery({
    queryKey: ["git-stash-diff", props.repoRoot, props.stash?.reference],
    queryFn: () => api.workspace.gitStashDiff(props.stash!.reference, props.repoRoot ?? undefined),
    enabled: Boolean(props.stash)
  });

  return (
    <Modal
      open={Boolean(props.stash)}
      title={props.stash?.reference ?? t("Stash Preview", "储藏预览")}
      description={props.stash?.subject}
      size="large"
      onClose={props.onClose}
      footer={<Button onClick={props.onClose}>{t("Close", "关闭")}</Button>}
    >
      <div className="git-stash-preview">
        {preview.isLoading && <div className="git-resource-state">{t("Loading stash changes...", "正在读取储藏改动…")}</div>}
        {preview.error && <div className="git-resource-state error">{preview.error.message}</div>}
        {preview.data?.stat && <pre className="git-diff-stat">{preview.data.stat}</pre>}
        {preview.data?.patch ? (
          <DiffView source={preview.data.patch} />
        ) : !preview.isLoading && !preview.error ? (
          <div className="git-resource-state">{t("No stash diff to display", "没有可显示的储藏差异")}</div>
        ) : null}
        {preview.data?.truncated && <div className="git-resource-state">{t("Diff truncated", "差异已截断")}</div>}
      </div>
    </Modal>
  );
}
