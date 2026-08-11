import { ChevronDown, ChevronRight } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import "./git-diff-stat.css";

/** 超过此行数时默认折叠，避免 80 文件的 stat 占满差异面板 */
const COLLAPSE_LINE_THRESHOLD = 6;

type GitDiffStatProps = {
  /** `git diff --stat` 原文 */
  stat: string;
};

/**
 * 渲染可折叠的 diffstat。
 *
 * 文件很多时默认只显示摘要行（如 `80 files changed…`），
 * 点击后再展开完整路径列表，避免 INDEX→WORKTREE 预览被大块文字顶满。
 *
 * @param props.stat git diff --stat 文本
 * @returns 折叠/展开的 stat 区块
 */
export function GitDiffStat(props: GitDiffStatProps) {
  const { t } = useI18n();
  const lines = useMemo(
    () => props.stat.replace(/\s+$/u, "").split("\n").filter((line) => line.length > 0),
    [props.stat]
  );
  const summaryLine = lines.at(-1) ?? "";
  const detailLines = lines.length > 1 ? lines.slice(0, -1) : [];
  const collapsible = lines.length > COLLAPSE_LINE_THRESHOLD;
  const [expanded, setExpanded] = useState(!collapsible);

  useEffect(() => {
    // 换一份 stat 时按新行数重置：大列表默认收起
    setExpanded(!collapsible);
  }, [collapsible, props.stat]);


  if (lines.length === 0) return null;

  if (!collapsible) {
    return <pre className="git-diff-stat">{props.stat}</pre>;
  }

  return (
    <div className={`git-diff-stat-block${expanded ? " is-expanded" : ""}`}>
      <Button
        className="git-diff-stat-toggle"
        onClick={() => setExpanded((value) => !value)}
        aria-expanded={expanded}
      >
        {expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        <span className="git-diff-stat-summary">{summaryLine}</span>
        <span className="git-diff-stat-hint">
          {expanded
            ? t("Collapse file list", "收起文件列表")
            : t(
                `Show ${detailLines.length} files`,
                `展开 ${detailLines.length} 个文件`
              )}
        </span>
      </Button>
      {expanded ? <pre className="git-diff-stat">{detailLines.join("\n")}</pre> : null}
    </div>
  );
}
