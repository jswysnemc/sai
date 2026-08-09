import { GitBranch, RefreshCw } from "lucide-react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { ServerDirectoryDialog } from "../../workspaces/server-directory-dialog";
import { CloneRepositoryDialog, type CloneRepositoryInput } from "../empty/clone-repository-dialog";
import { SourceControlEmptyState } from "../empty/source-control-empty-state";
import { GitOutputPanel } from "../output/git-output-panel";
import type { GitOutputEntry } from "../types";

type GitSetupViewProps = {
  initBranch: string;
  onInitBranchChange: (branch: string) => void;
  busy: boolean;
  onInitialize: () => void;
  onRefresh: () => void;
  outputEntries: GitOutputEntry[];
  errorMessage: string | null;
  cloneDialogOpen: boolean;
  onCloneDialogOpenChange: (open: boolean) => void;
  onCloneContinue: (input: CloneRepositoryInput) => void;
  cloneTargetOpen: boolean;
  onCloneTargetClose: () => void;
  onCloneTargetSelect: (parent: string) => Promise<void>;
  openFolderDialogOpen: boolean;
  onOpenFolderDialogOpenChange: (open: boolean) => void;
  onOpenFolder: (path: string) => Promise<void>;
};

/**
 * 渲染尚未建立 Git 仓库时的引导视图。
 *
 * 提供初始化、打开已有目录与克隆三条入口，并保留操作输出便于排查失败原因。
 *
 * @param props 初始化参数、对话框开关与操作回调
 * @returns 引导视图
 */
export function GitSetupView(props: GitSetupViewProps) {
  const { t } = useI18n();
  return (
    <section className="diff-pane git-manager git-review">
      <header className="panel-head">
        <div>
          <span className="eyebrow">{t("Git workspace", "Git 工作区")}</span>
          <h2>
            <GitBranch size={15} />
            {t("Version control", "版本管理")}
          </h2>
        </div>
        <Button className="icon-button" onClick={props.onRefresh} aria-label={t("Refresh", "刷新")}>
          <RefreshCw size={14} />
        </Button>
      </header>
      <SourceControlEmptyState
        branch={props.initBranch}
        busy={props.busy}
        onBranchChange={props.onInitBranchChange}
        onInitialize={props.onInitialize}
        onOpenFolder={() => props.onOpenFolderDialogOpenChange(true)}
        onClone={() => props.onCloneDialogOpenChange(true)}
      />
      <GitOutputPanel entries={props.outputEntries} />
      {props.errorMessage && <div className="pane-error">{props.errorMessage}</div>}
      <CloneRepositoryDialog
        open={props.cloneDialogOpen}
        onClose={() => props.onCloneDialogOpenChange(false)}
        onContinue={props.onCloneContinue}
      />
      <ServerDirectoryDialog
        open={props.openFolderDialogOpen}
        onClose={() => props.onOpenFolderDialogOpenChange(false)}
        onSelect={props.onOpenFolder}
      />
      <ServerDirectoryDialog
        open={props.cloneTargetOpen}
        title={t("Choose Clone Destination", "选择克隆目标目录")}
        description={t(
          "Choose the parent directory where Git will create the cloned repository folder.",
          "选择父目录，Git 将在其中创建克隆仓库文件夹。"
        )}
        currentLabel={t("Clone into current directory", "克隆到当前目录")}
        pendingLabel={t("Cloning Repository", "正在克隆仓库")}
        onClose={props.onCloneTargetClose}
        onSelect={props.onCloneTargetSelect}
      />
    </section>
  );
}
