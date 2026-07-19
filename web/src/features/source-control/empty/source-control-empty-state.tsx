import { Download, FolderOpen, GitBranch, Plus } from "lucide-react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import "./source-control-empty-state.css";

type SourceControlEmptyStateProps = {
  branch: string;
  busy: boolean;
  onBranchChange: (branch: string) => void;
  onInitialize: () => void;
  onOpenFolder: () => void;
  onClone: () => void;
};

/**
 * 渲染未检测到仓库时的 Source Control 操作入口。
 *
 * @param props 默认分支、忙碌状态与仓库入口回调
 * @returns 紧凑的仓库空状态
 */
export function SourceControlEmptyState(props: SourceControlEmptyStateProps) {
  const { t } = useI18n();

  return (
    <section className="source-control-empty-state">
      <header>
        <GitBranch size={20} />
        <span>
          <strong>{t("No Git repository detected", "未检测到 Git 仓库")}</strong>
          <small>{t("Open an existing repository, clone one, or initialize this workspace.", "打开现有仓库、克隆仓库，或初始化当前工作区。")}</small>
        </span>
      </header>
      <div className="source-control-empty-actions">
        <Button variant="primary" disabled={props.busy} onClick={props.onOpenFolder}>
          <FolderOpen size={14} />
          {t("Open Folder", "打开文件夹")}
        </Button>
        <Button disabled={props.busy} onClick={props.onClone}>
          <Download size={14} />
          {t("Clone Repository", "克隆仓库")}
        </Button>
      </div>
      <div className="source-control-init-row">
        <label>
          <span>{t("Default branch", "默认分支")}</span>
          <input
            value={props.branch}
            onChange={(event) => props.onBranchChange(event.target.value)}
            spellCheck={false}
          />
        </label>
        <Button
          disabled={props.busy || !props.branch.trim()}
          onClick={props.onInitialize}
        >
          <Plus size={14} />
          {t("Initialize Repository", "初始化仓库")}
        </Button>
      </div>
    </section>
  );
}
