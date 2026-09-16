import { apiRequest } from "./api-request";

export type GitDiffStats = {
  added: number;
  removed: number;
};

/**
 * 【工作概览】【统计查询】获取轻量增删行数，避免传输完整补丁。
 * @param repoRoot 可选的已选仓库根目录
 * @returns 工作树相对 HEAD 的增删行数
 */
export function fetchGitDiffStats(repoRoot?: string): Promise<GitDiffStats> {
  const query = new URLSearchParams();
  if (repoRoot) query.set("repo_root", repoRoot);
  const parameters = query.toString();
  return apiRequest<GitDiffStats>(`/api/workspace/git/diff-stats${parameters ? `?${parameters}` : ""}`);
}
