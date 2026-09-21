import type { QueryClient } from "@tanstack/react-query";

/**
 * 【工作区导航】【缓存刷新】刷新切换工作区后受影响的查询。
 *
 * @param client 当前页面查询客户端
 * @returns 所有相关查询失效完成后的 Promise
 */
export function invalidateWorkspaceContext(client: QueryClient): Promise<void> {
  return Promise.all([
    client.invalidateQueries({ queryKey: ["workspaces"] }),
    client.invalidateQueries({ queryKey: ["sessions"] }),
    client.invalidateQueries({ queryKey: ["session-tree"] }),
    client.invalidateQueries({ queryKey: ["file-tree"] }),
    client.invalidateQueries({ queryKey: ["workspace-diff"] }),
    client.invalidateQueries({ queryKey: ["runtime-overview"] })
  ]).then(() => undefined);
}
