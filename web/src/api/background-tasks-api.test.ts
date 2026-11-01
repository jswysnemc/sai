import { afterEach, expect, it, vi } from "vitest";
import { apiRequest } from "./api-request";
import { backgroundTasksApi } from "./background-tasks-api";

vi.mock("./api-request", () => ({ apiRequest: vi.fn().mockResolvedValue({}) }));
afterEach(() => vi.clearAllMocks());

it("普通停止兼容原路径，只有显式强制停止才发送 force 参数", async () => {
  await backgroundTasksApi.stop("task/a");
  expect(apiRequest).toHaveBeenLastCalledWith("/api/background-tasks/task%2Fa/stop", { method: "POST" });
  await backgroundTasksApi.stop("task/a", true);
  expect(apiRequest).toHaveBeenLastCalledWith("/api/background-tasks/task%2Fa/stop?force=true", { method: "POST" });
});

it("日志读取传递尾部行数，清理默认保留日志", async () => {
  await backgroundTasksApi.output("task", 2000);
  expect(apiRequest).toHaveBeenLastCalledWith("/api/background-tasks/task/output?tail_lines=2000");
  await backgroundTasksApi.cleanup();
  expect(apiRequest).toHaveBeenLastCalledWith("/api/background-tasks?remove_logs=false", { method: "DELETE" });
});
