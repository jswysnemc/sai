export type SourceControlDiffView = "changes" | "branch";
export type GitReviewDiffMode = "working_tree" | "branch";

/**
 * 根据当前视图选择 Git 比较模式。
 *
 * 变更视图使用 working_tree 一次取回全部未提交差异（含已暂存与未跟踪文件），
 * 审阅区因此能像 Cursor 一样把所有文件的差异平铺成一条卡片流；
 * 分支视图保持与基线比较。
 *
 * @param view 用户选择的变更或分支视图
 * @returns 后端 Git Diff 模式
 */
export function resolveGitReviewDiffMode(view: SourceControlDiffView): GitReviewDiffMode {
  return view === "branch" ? "branch" : "working_tree";
}
