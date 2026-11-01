import { apiRequest } from "./api-request";
import type { BackgroundTask, BackgroundTaskOutput } from "./contracts";

/** 后台任务接口独立维护，继续通过统一客户端导出。 */
export const backgroundTasksApi = {
  /** 获取任务列表，返回服务端已刷新状态的任务集合。 */
  list: () => apiRequest<{ tasks: BackgroundTask[] }>("/api/background-tasks"),
  /** 读取 id 对应任务的日志尾部，tailLines 为每个流的最大行数，返回输出与截断标记。 */
  output: (id: string, tailLines = 200) =>
    apiRequest<BackgroundTaskOutput>(`/api/background-tasks/${encodeURIComponent(id)}/output?tail_lines=${tailLines}`),
  /** 停止 id 对应任务，force 为真时立即强制终止，返回任务状态和原运行状态。 */
  stop: (id: string, force = false) => apiRequest<{ task: BackgroundTask; was_running: boolean }>(
    `/api/background-tasks/${encodeURIComponent(id)}/stop${force ? "?force=true" : ""}`, { method: "POST" }
  ),
  /** 清理已结束记录，removeLogs 指定是否同时删除日志，返回实际移除标识和剩余数量。 */
  cleanup: (removeLogs = false) =>
    apiRequest<{ removed: string[]; remaining: number }>(`/api/background-tasks?remove_logs=${removeLogs}`, { method: "DELETE" })
};
