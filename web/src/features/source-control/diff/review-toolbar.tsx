import { ChevronDown, ChevronUp, ChevronsDownUp, ChevronsUpDown, GitBranch, Search, X } from "lucide-react";
import { Button } from "../../../shared/ui/button/button";
import { TextInput } from "../../../shared/ui/form/text-input";
import { useI18n } from "../../i18n/use-i18n";
import { DiffViewControls } from "../../chat/tool-renderers/diff/diff-view-controls";
import type { useDiffViewOptions } from "../../chat/tool-renderers/diff/use-diff-view-options";
import "./review-toolbar.css";

type ReviewToolbarProps = {
  title: string;
  branch: string;
  added: number;
  removed: number;
  fileCount: number;
  visibleCount: number;
  currentIndex: number;
  query: string;
  allCollapsed: boolean;
  truncated: boolean;
  display: ReturnType<typeof useDiffViewOptions>;
  onQueryChange: (value: string) => void;
  onNavigate: (direction: -1 | 1) => void;
  onToggleAll: () => void;
};

/**
 * 汇总变更数量并提供文件筛选、前后导航及显示设置。
 * @param props 当前审阅范围、文件位置、筛选条件与操作回调
 * @returns 两行紧凑审阅工具栏
 */
export function ReviewToolbar(props: ReviewToolbarProps) {
  const { t } = useI18n();
  return <header className="git-review-summary">
    <div className="git-review-overview">
      <strong className="git-review-summary-title">{props.title}</strong>
      <span className="git-review-summary-stats"><b>+{props.added}</b><i>-{props.removed}</i></span>
      {props.truncated && <span className="git-review-partial" title={t("Counts cover the loaded diff; individual files can be loaded separately", "统计仅包含已载入的差异，可单独读取文件")}>{t("Partial", "部分")}</span>}
      <span className="git-review-summary-branch" title={props.branch}><GitBranch size={12} aria-hidden />{props.branch}</span>
      <DiffViewControls options={props.display} />
    </div>
    <div className="git-review-navigation">
      <label className="git-review-filter">
        <Search size={13} aria-hidden />
        <TextInput value={props.query} onChange={(event) => props.onQueryChange(event.target.value)}
          placeholder={t(`Filter ${props.fileCount} files`, `筛选 ${props.fileCount} 个文件`)} aria-label={t("Filter changed files", "筛选变更文件")} />
        {props.query && <Button variant="ghost" size="icon" onClick={() => props.onQueryChange("")}
          aria-label={t("Clear file filter", "清除文件筛选")}><X size={12} /></Button>}
      </label>
      <div className="git-review-file-navigation" role="group" aria-label={t("Navigate files", "文件导航")}>
        <Button variant="ghost" size="icon" onClick={() => props.onNavigate(-1)} disabled={props.currentIndex <= 0}
          aria-label={t("Previous file", "上一个文件")} title={t("Previous file", "上一个文件")}><ChevronUp size={14} /></Button>
        <span aria-live="polite">{props.visibleCount ? props.currentIndex + 1 : 0}/{props.visibleCount}</span>
        <Button variant="ghost" size="icon" onClick={() => props.onNavigate(1)} disabled={props.currentIndex >= props.visibleCount - 1}
          aria-label={t("Next file", "下一个文件")} title={t("Next file", "下一个文件")}><ChevronDown size={14} /></Button>
      </div>
      <Button variant="ghost" size="icon" className="git-review-collapse" onClick={props.onToggleAll} disabled={props.visibleCount === 0}
        aria-label={props.allCollapsed ? t("Expand all files", "展开全部文件") : t("Collapse all files", "折叠全部文件")}
        title={props.allCollapsed ? t("Expand all files", "展开全部文件") : t("Collapse all files", "折叠全部文件")}>
        {props.allCollapsed ? <ChevronsUpDown size={14} /> : <ChevronsDownUp size={14} />}
      </Button>
    </div>
  </header>;
}
