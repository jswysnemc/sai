import { useQuery } from "@tanstack/react-query";
import { api } from "../../../api/client";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";
import { RemoteSection } from "./remote-section";
import { StashSection } from "./stash-section";
import { TagSection } from "./tag-section";

type RepositoryResourcesProps = {
  repoRoot: string | null;
  open: boolean;
  busy: boolean;
  runOperation: RunGitOperation;
};

/**
 * 查询并组合 stash、标签和远端资源分区。
 *
 * @param props 仓库、菜单状态和 Git 操作回调
 * @returns 仓库资源菜单分组
 */
export function RepositoryResources(props: RepositoryResourcesProps) {
  const { t } = useI18n();
  const resources = useQuery({
    queryKey: ["git-resources", props.repoRoot],
    queryFn: () => api.workspace.gitResources(props.repoRoot ?? undefined),
    enabled: props.open,
    staleTime: 5_000
  });

  if (resources.isLoading) return <div className="git-resource-state">{t("Loading repository resources...", "正在读取仓库资源…")}</div>;
  if (resources.error) return <div className="git-resource-state error">{resources.error.message}</div>;

  return (
    <>
      <StashSection
        stashes={resources.data?.stashes ?? []}
        repoRoot={props.repoRoot}
        busy={props.busy}
        runOperation={props.runOperation}
      />
      <TagSection
        tags={resources.data?.tags ?? []}
        busy={props.busy}
        runOperation={props.runOperation}
      />
      <RemoteSection
        remotes={resources.data?.remotes ?? []}
        busy={props.busy}
        runOperation={props.runOperation}
      />
    </>
  );
}
