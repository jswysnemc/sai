import { QueryClient, QueryClientProvider, QueryObserver } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api/client";
import type { Session, WorkspaceSessions } from "../../api/contracts";
import { useSessionActions } from "./use-session-actions";

const cleanup: (() => void)[] = [];
afterEach(() => {
  cleanup.splice(0).forEach((dispose) => dispose());
  vi.restoreAllMocks();
});

/** 创建可手动结束的接口请求，返回等待 Promise 和完成方法。 */
function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((accept) => { resolve = accept; });
  return { promise, resolve };
}

/**
 * 创建真实查询缓存与会话操作，使用受控接口模拟服务端当前会话。
 * @returns 操作实例、查询缓存、接口完成开关与当前选中项读取方法
 */
function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } });
  let activeId = "A";
  const gates = { A: deferred(), B: deferred(), C: deferred() };
  /** 根据服务端活动指针生成会话列表，返回三个会话。 */
  const sessions = (): Session[] => ["A", "B", "C"].map((id) => ({
    id, title: id, active: id === activeId, created_at: "now", updated_at: "now"
  }));
  /** 根据服务端活动指针生成侧栏树，返回当前工作区。 */
  const tree = (): WorkspaceSessions[] => [{
    workspace_id: "workspace", workspace_name: "Project", workspace_path: "/tmp/project",
    is_git_repository: false, active: true, sessions: sessions()
  }];
  vi.spyOn(api.sessions, "switch").mockImplementation(async (id) => {
    await gates[id as keyof typeof gates].promise;
    activeId = id;
    return sessions().find((session) => session.id === id)!;
  });
  vi.spyOn(api.sessions, "list").mockImplementation(async () => sessions());
  vi.spyOn(api.sessions, "tree").mockImplementation(async () => tree());
  client.setQueryData(["sessions"], sessions());
  client.setQueryData(["session-tree"], tree());
  for (const query of [
    { queryKey: ["sessions"], queryFn: api.sessions.list },
    { queryKey: ["session-tree"], queryFn: api.sessions.tree }
  ]) {
    const observer = new QueryObserver<Session[] | WorkspaceSessions[]>(client, { ...query, refetchOnMount: false });
    cleanup.push(observer.subscribe(() => {}));
  }
  cleanup.push(() => client.clear());
  let actions!: ReturnType<typeof useSessionActions>;
  const onNavigate = vi.fn();
  /** 获取真实会话操作实例，返回空渲染结果。 */
  function Probe() {
    actions = useSessionActions({
      confirm: async () => true, t: (_en, zh) => zh,
      tree: () => client.getQueryData<WorkspaceSessions[]>(["session-tree"]), onNavigate
    });
    return null;
  }
  renderToStaticMarkup(<QueryClientProvider client={client}><Probe /></QueryClientProvider>);
  return {
    actions, client, gates, tree, onNavigate,
    activeId: () => activeId,
    displayedId: () => client.getQueryData<Session[]>(["sessions"])?.find((session) => session.active)?.id,
    sidebarId: () => client.getQueryData<WorkspaceSessions[]>(["session-tree"])?.[0].sessions.find((session) => session.active)?.id
  };
}

describe("会话切换并发", () => {
  it("快速选择 B、C 时，较晚返回的 B 不覆盖最后选择的 C", async () => {
    const test = setup();
    const first = test.actions.openSession("workspace", "B", true, false);
    await vi.waitFor(() => expect(api.sessions.switch).toHaveBeenCalledWith("B"));
    const last = test.actions.openSession("workspace", "C", true, false);
    test.gates.C.resolve();
    await new Promise((resolve) => setTimeout(resolve, 20));
    test.gates.B.resolve();
    await Promise.all([first, last]);
    expect(test.activeId()).toBe("C");
    expect(test.displayedId()).toBe("C");
    expect(test.sidebarId()).toBe("C");
  });

  it("A 切到 B 的过程中再次选择 A，应保留最后一次选择", async () => {
    const test = setup();
    const first = test.actions.openSession("workspace", "B", true, false);
    await vi.waitFor(() => expect(api.sessions.switch).toHaveBeenCalledWith("B"));
    const last = test.actions.openSession("workspace", "A", true, true);
    test.gates.A.resolve();
    test.gates.B.resolve();
    await Promise.all([first, last]);
    expect(test.activeId()).toBe("A");
    expect(test.displayedId()).toBe("A");
  });

  it("侧栏刷新较慢时，消息区和侧栏仍选择同一会话", async () => {
    const test = setup();
    const treeGate = deferred();
    vi.mocked(api.sessions.tree).mockImplementation(async () => {
      await treeGate.promise;
      return test.tree();
    });
    const switchRequest = test.actions.openSession("workspace", "B", true, false);
    test.gates.B.resolve();
    await vi.waitFor(() => expect(test.displayedId()).toBe("B"));
    const sidebarId = test.sidebarId();
    treeGate.resolve();
    await switchRequest;
    expect(sidebarId).toBe("B");
  });
});
