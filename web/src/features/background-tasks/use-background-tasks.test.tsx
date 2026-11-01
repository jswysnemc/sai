import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api/client";
import type { BackgroundTask } from "../../api/contracts";
import { useBackgroundTasks } from "./use-background-tasks";
import type { BackgroundTaskFilter } from "./background-task-utils";

const running: BackgroundTask = {
  id: "running", label: "Watch", command: "watch", cwd: "/tmp", pid: 10,
  status: "running", started_at: 100, updated_at: 100, timeout_seconds: 600
};
const finished: BackgroundTask = { ...running, id: "finished", status: "exited" };
const clients: QueryClient[] = [];

afterEach(() => {
  clients.splice(0).forEach((client) => client.clear());
  vi.restoreAllMocks();
});

/**
 * 创建真实查询缓存及任务管理方法，接口请求由各测试控制。
 * @param selectedId 初始选择的任务标识
 * @returns 查询缓存及重新读取管理状态的方法
 */
function setup(selectedId: string | null = null) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } });
  clients.push(client);
  client.setQueryData(["background-tasks"], { tasks: [finished, running] });
  /** 读取当前缓存对应的管理状态，返回钩子实例。 */
  const read = (id = selectedId, filter: BackgroundTaskFilter = "all") => {
    let manager!: ReturnType<typeof useBackgroundTasks>;
    /** 在查询上下文中读取管理方法，返回空渲染结果。 */
    function Probe() {
      manager = useBackgroundTasks(id, filter);
      return null;
    }
    renderToStaticMarkup(<QueryClientProvider client={client}><Probe /></QueryClientProvider>);
    return manager;
  };
  return { client, read };
}

describe("后台任务查询与操作", () => {
  it("默认选择运行中的任务，保留有效选择并修复失效选择", () => {
    const { read } = setup();
    expect(read().selected?.id).toBe("running");
    expect(read("finished").selected?.id).toBe("finished");
    expect(read("removed").selected?.id).toBe("running");
    expect(read("running", "finished").selected?.id).toBe("finished");
    expect(read("finished", "running").visibleTasks.map((task) => task.id)).toEqual(["running"]);
  });

  it("强制停止显式传递 force，不改变普通停止的默认语义", async () => {
    const stop = vi.spyOn(api.backgroundTasks, "stop").mockResolvedValue({ task: { ...running, status: "stopped" }, was_running: true });
    const { read } = setup();
    await read().stop({ id: "running", force: true });
    expect(stop).toHaveBeenCalledWith("running", true);
  });

  it("停止后同步列表状态并使旧输出失效", async () => {
    const stopped = { ...running, status: "stopped", updated_at: 120 };
    vi.spyOn(api.backgroundTasks, "stop").mockResolvedValue({ task: stopped, was_running: true });
    const { client, read } = setup("running");
    client.setQueryData(["background-task-output", "running", "running"], { stdout: "old" });
    await read().stop("running");
    expect(read().selected?.status).toBe("stopped");
    expect(client.getQueryState(["background-task-output", "running", "running"])?.isInvalidated).toBe(true);
    expect(read().output).toBeUndefined();
    expect(read().outputLoading).toBe(true);
  });

  it("清理仅移除服务端返回的记录及其输出缓存，保留日志和运行任务", async () => {
    const cleanup = vi.spyOn(api.backgroundTasks, "cleanup").mockResolvedValue({ removed: ["finished"], remaining: 1 });
    const { client, read } = setup("finished");
    client.setQueryData(["background-task-output", "finished", "exited"], { stdout: "finished" });
    client.setQueryData(["background-task-output", "running", "running"], { stdout: "running" });
    await read().cleanup();
    expect(cleanup).toHaveBeenCalledWith(false);
    expect(read().tasks.map((task) => task.id)).toEqual(["running"]);
    expect(read().selected?.id).toBe("running");
    expect(client.getQueryData(["background-task-output", "finished", "exited"])).toBeUndefined();
    expect(read().output).toEqual({ stdout: "running" });
  });

  it("最后一条记录清理后清空选择，不再读取被移除任务", async () => {
    vi.spyOn(api.backgroundTasks, "cleanup").mockResolvedValue({ removed: ["finished"], remaining: 0 });
    const { client, read } = setup("finished");
    client.setQueryData(["background-tasks"], { tasks: [finished] });
    await read().cleanup();
    expect(read().selected).toBeUndefined();
    expect(read().output).toBeUndefined();
    expect(read().outputLoading).toBe(false);
  });

  it("清理时取消旧列表请求，迟到响应不能恢复已删除记录", async () => {
    vi.spyOn(api.backgroundTasks, "cleanup").mockResolvedValue({ removed: ["finished"], remaining: 1 });
    const { client, read } = setup("finished");
    const manager = read();
    let resolve!: (value: { tasks: BackgroundTask[] }) => void;
    const pending = client.fetchQuery({
      queryKey: ["background-tasks"],
      queryFn: () => new Promise<{ tasks: BackgroundTask[] }>((accept) => { resolve = accept; })
    }).catch(() => undefined);
    await manager.cleanup();
    resolve({ tasks: [finished, running] });
    await pending;
    expect(read().tasks.map((task) => task.id)).toEqual(["running"]);
  });

  it("停止失败不修改记录，手动刷新错误返回给提示层", async () => {
    vi.spyOn(api.backgroundTasks, "stop").mockRejectedValue(new Error("stop failed"));
    vi.spyOn(api.backgroundTasks, "list").mockRejectedValue(new Error("offline"));
    const { read } = setup();
    await expect(read().stop("running")).rejects.toThrow("stop failed");
    expect(read().selected?.status).toBe("running");
    await expect(read().refresh()).rejects.toThrow("offline");
  });
});
