import type { ChangeSectionKind } from "../changes/change-section";

export type SourceControlDiffView = "changes" | "branch";
export type GitReviewDiffMode = "unstaged" | "staged" | "branch";

/**
 * 根据当前视图与文件分区选择正确的 Git 比较模式。
 *
 * @param view 用户选择的变更或分支视图
 * @param section 当前文件所属分区
 * @returns 后端 Git Diff 模式
 */
export function resolveGitReviewDiffMode(
  view: SourceControlDiffView,
  section: ChangeSectionKind
): GitReviewDiffMode {
  if (view === "branch") return "branch";
  return section === "staged" ? "staged" : "unstaged";
}
