/** Git 文件监听事件。 */
export type GitWatchEvent = {
  sequence: number;
  workspace_root: string;
  paths: string[];
  paths_truncated: boolean;
  repository_metadata_changed: boolean;
  error?: string | null;
};

/** Git SSE 支持的事件类型。 */
export const GIT_EVENT_TYPES = ["git.changed", "git.error"] as const;

/**
 * 构造 Git 仓库事件订阅地址。
 *
 * @param repoRoot 可选仓库或 worktree 根目录
 * @returns SSE 地址
 */
export function gitEventsUrl(repoRoot: string | null): string {
  if (!repoRoot) return "/api/workspace/git/events";
  return `/api/workspace/git/events?repo_root=${encodeURIComponent(repoRoot)}`;
}

/**
 * 解析并校验 Git 仓库事件。
 *
 * @param raw SSE 原始 JSON 数据
 * @returns Git 文件监听事件
 */
export function parseGitWatchEvent(raw: string): GitWatchEvent {
  const value = JSON.parse(raw) as Partial<GitWatchEvent>;
  if (
    typeof value.sequence !== "number"
    || typeof value.workspace_root !== "string"
    || !Array.isArray(value.paths)
    || value.paths.some((path) => typeof path !== "string")
    || typeof value.paths_truncated !== "boolean"
    || typeof value.repository_metadata_changed !== "boolean"
  ) {
    throw new Error("invalid Git repository event payload");
  }
  return value as GitWatchEvent;
}
