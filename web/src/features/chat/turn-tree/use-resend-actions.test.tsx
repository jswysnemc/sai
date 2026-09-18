import { QueryClient, QueryClientProvider, QueryObserver } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../../api/client";
import type { SessionTimeline } from "../../../api/contracts";
import { useResendActions } from "./use-resend-actions";

const history: SessionTimeline = {
  turns: [{
    turn_id: "failed-turn", seq: 1, status: "failed", automatic: false,
    user: { timestamp: "now", content: "原始问题" },
    assistant: { timestamp: "now", content: "" }, tools: []
  }]
};
const cleanup: (() => void)[] = [];

afterEach(() => {
  cleanup.splice(0).forEach((dispose) => dispose());
  vi.restoreAllMocks();
});

/**
 * 创建由测试控制完成时机的请求。
 * @returns 请求 Promise 与成功、失败回调
 */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

/**
 * 在真实查询上下文中取得重发动作，并订阅实际页面使用的查询。
 * @param timelineRequest 分支变化后的时间线请求
 * @param metadataRequest 辅助列表和分支树请求
 * @returns 动作、查询缓存及运行回调
 */
function setup(
  timelineRequest = () => Promise.resolve<SessionTimeline>({ turns: [] }),
  metadataRequest = () => Promise.resolve<unknown>([])
) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } });
  const options = {
    sessionId: "session", running: false, mode: "yolo" as const, thinkingLevel: "auto" as const,
    moveToParent: vi.fn(async () => true), resetRun: vi.fn(),
    startRun: vi.fn(async (_content: string, _images?: string[]) => {}), onError: vi.fn()
  };
  client.setQueryData(["timeline", "session"], history);
  const queries = [
    { queryKey: ["timeline", "session"], queryFn: timelineRequest },
    { queryKey: ["session-turn-tree", "session"], queryFn: metadataRequest },
    { queryKey: ["sessions"], queryFn: metadataRequest },
    { queryKey: ["session-tree"], queryFn: metadataRequest }
  ];
  for (const query of queries) {
    if (query.queryKey[0] !== "timeline") client.setQueryData(query.queryKey, []);
    const observer = new QueryObserver(client, { ...query, refetchOnMount: false });
    cleanup.push(observer.subscribe(() => {}));
  }
  cleanup.push(() => client.clear());
  let actions!: ReturnType<typeof useResendActions>;
  /** 获取同一挂载实例持有的动作与引用，返回空渲染结果。 */
  function Probe() { actions = useResendActions(options); return null; }
  renderToStaticMarkup(<QueryClientProvider client={client}><Probe /></QueryClientProvider>);
  vi.spyOn(api.sessions, "timeline").mockResolvedValue(history);
  return { actions, client, options };
}

describe("useResendActions", () => {
  it("已加载轮次重试只等待新分支时间线，辅助列表慢请求不阻塞运行", async () => {
    const timeline = deferred<SessionTimeline>();
    const metadata = deferred<unknown>();
    const timelineRequest = vi.fn(() => timeline.promise);
    const { actions, options, client } = setup(timelineRequest, () => metadata.promise);
    const retry = actions.retry("原始问题", undefined, "failed-turn");

    await vi.waitFor(() => expect(timelineRequest).toHaveBeenCalledOnce());
    expect(options.startRun).not.toHaveBeenCalled();
    timeline.resolve({ turns: [] });
    await vi.waitFor(() => expect(options.startRun).toHaveBeenCalledOnce(), { timeout: 300 });
    await retry;
    expect(api.sessions.timeline).not.toHaveBeenCalled();
    expect(client.getQueryData(["timeline", "session"])).toEqual({ turns: [] });
    expect(options.moveToParent).toHaveBeenCalledExactlyOnceWith("failed-turn");
    metadata.resolve([]);
  });

  it("连续点击重试或同时编辑重发只移动一次分支并启动一次运行", async () => {
    const move = deferred<boolean>();
    const { actions, options } = setup();
    options.moveToParent.mockImplementation(() => move.promise);
    const requests = [
      actions.retry("原始问题", undefined, "failed-turn"),
      actions.retry("原始问题", undefined, "failed-turn"),
      actions.editAndResend("failed-turn", "修改问题", [])
    ];
    await vi.waitFor(() => expect(options.moveToParent).toHaveBeenCalled());
    move.resolve(true);
    await Promise.all(requests);
    expect(options.moveToParent).toHaveBeenCalledOnce();
    expect(options.startRun).toHaveBeenCalledExactlyOnceWith("原始问题", undefined);
  });

  it("实时轮次尚未进入缓存时读取最新时间线后再退回父轮次", async () => {
    const { actions, client, options } = setup();
    client.setQueryData(["timeline", "session"], { turns: [] });
    await actions.retry("原始问题", ["image.png"], "failed-turn");
    expect(api.sessions.timeline).toHaveBeenCalledExactlyOnceWith("session");
    expect(options.moveToParent).toHaveBeenCalledExactlyOnceWith("failed-turn");
    expect(options.startRun).toHaveBeenCalledExactlyOnceWith("原始问题", ["image.png"]);
  });

  it("运行创建前失败的重试不退回无关历史，编辑缺失轮次则拒绝提交", async () => {
    const { actions, options } = setup();
    await actions.retry("新问题", undefined, "preflight-failure");
    expect(options.moveToParent).not.toHaveBeenCalled();
    expect(options.startRun).toHaveBeenCalledExactlyOnceWith("新问题", undefined);
    await actions.editAndResend("missing-turn", "修改问题", []);
    expect(options.startRun).toHaveBeenCalledOnce();
    expect(options.onError).toHaveBeenCalledOnce();
  });

  it("分支时间线加载失败时不启动运行，并允许随后再次重试", async () => {
    const timelineRequest = vi.fn<() => Promise<SessionTimeline>>()
      .mockRejectedValueOnce(new Error("读取失败"))
      .mockResolvedValue({ turns: [] });
    const { actions, options } = setup(timelineRequest);
    await actions.retry("原始问题", undefined, "failed-turn");
    expect(options.startRun).not.toHaveBeenCalled();
    expect(options.onError).toHaveBeenCalledOnce();
    await actions.retry("原始问题", undefined, "failed-turn");
    expect(options.startRun).toHaveBeenCalledOnce();
  });
});
