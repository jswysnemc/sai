import { afterEach, describe, expect, it, vi } from "vitest";
import type { RunInfo, WebEvent } from "../../api/contracts";
import { createReplyNotifier } from "./reply-notification";
import type { NotificationMessage } from "./notification-client";

const notification: NotificationMessage = { title: "Custom", body: "Lua result", desktop: true, sound: false };
const notifiers: ReturnType<typeof createReplyNotifier>[] = [];

/**
 * 【通知测试】【事件样本】创建具有真实作用域和交付序号的终态事件。
 * @param changes 覆盖字段
 * @returns 可交给正式消费者的事件
 */
function event(changes: Partial<WebEvent> = {}): WebEvent {
  return {
    sequence: 2, run_id: "run-1", workspace_id: "workspace", session_id: "session",
    timestamp: "2026-09-10T00:00:00Z", type: "run.completed", payload: {}, replayed: false,
    ...changes
  };
}

/**
 * 【通知测试】【隔离实例】替换网络与平台投递，保留正式去重及取消逻辑。
 * @param plan 可选的延迟或失败策略接口
 * @returns 消费者及可观察的依赖
 */
function fixture(plan = vi.fn(async () => [notification])) {
  const deliver = vi.fn();
  const notifier = createReplyNotifier("workspace", "session", { plan, deliver });
  notifiers.push(notifier);
  return { notifier, plan, deliver };
}

afterEach(() => {
  for (const notifier of notifiers.splice(0)) notifier.dispose();
  vi.useRealTimers();
});

describe("reply notification consumption", () => {
  it("consumes a run before awaiting its plan and ignores repeated terminal kinds", async () => {
    let resolve!: (value: NotificationMessage[]) => void;
    const pending = new Promise<NotificationMessage[]>((done) => { resolve = done; });
    const { notifier, plan, deliver } = fixture(vi.fn(() => pending));
    const first = notifier.accept(event(), "zh-CN");
    await notifier.accept(event({ type: "run.failed", sequence: 3 }), "zh-CN");
    await notifier.accept(event({ sequence: 4, replayed: true }), "en-US");
    expect(plan).toHaveBeenCalledTimes(1);
    resolve([notification]);
    await first;
    expect(deliver).toHaveBeenCalledTimes(1);
    expect(deliver.mock.calls[0][0]).toEqual(notification);
    expect(plan.mock.calls[0]).toEqual(["completed", "zh-CN", expect.any(AbortSignal)]);
  });

  it("restores historical messages without replaying their notification effects", async () => {
    const { notifier, plan } = fixture();
    await notifier.accept(event({ type: "run.started", sequence: 1, replayed: true }), "en-US");
    await notifier.accept(event({ replayed: true }), "en-US");
    await notifier.accept(event({ sequence: 3 }), "en-US");
    expect(plan).not.toHaveBeenCalled();
  });

  it("notifies once when a known live run completes during an SSE disconnection", async () => {
    const { notifier, plan, deliver } = fixture();
    await notifier.accept(event({ type: "run.started", sequence: 1 }), "en-US");
    await notifier.accept(event({ replayed: true, type: "run.interrupted" }), "en-US");
    await notifier.accept(event({ replayed: true, sequence: 3 }), "en-US");
    expect(plan).toHaveBeenCalledTimes(1);
    expect(plan).toHaveBeenNthCalledWith(1, "interrupted", "en-US", expect.any(AbortSignal));
    expect(deliver).toHaveBeenCalledTimes(1);
  });

  it("uses the host active-run list to recognize a completion missed during attachment", async () => {
    const { notifier, plan } = fixture();
    const run: RunInfo = {
      run_id: "run-1", workspace_id: "workspace", session_id: "session", status: "running"
    };
    notifier.trackActiveRuns([run, { ...run, run_id: "foreign", workspace_id: "other" }]);
    await notifier.accept(event({ replayed: true }), "en-US");
    await notifier.accept(event({ run_id: "foreign", sequence: 3, replayed: true }), "en-US");
    expect(plan).toHaveBeenCalledTimes(1);
  });

  it("keeps workspace, session and delivery sequence boundaries independent", async () => {
    const { notifier, plan } = fixture();
    await notifier.accept(event({ workspace_id: "other", sequence: 900 }), "en-US");
    await notifier.accept(event({ session_id: "other", sequence: 901 }), "en-US");
    await notifier.accept(event(), "en-US");
    await notifier.accept(event({ run_id: "old", sequence: 1 }), "en-US");
    await notifier.accept(event({ run_id: "next", sequence: 3, type: "run.failed" }), "zh-CN");
    expect(plan).toHaveBeenCalledTimes(2);
    expect(plan).toHaveBeenNthCalledWith(2, "failed", "zh-CN", expect.any(AbortSignal));
  });

  it("drops late plan results and cancels outstanding work when the session closes", async () => {
    let resolve!: (value: NotificationMessage[]) => void;
    let signal!: AbortSignal;
    const plan = vi.fn((_status, _locale, suppliedSignal: AbortSignal) => {
      signal = suppliedSignal;
      return new Promise<NotificationMessage[]>((done) => { resolve = done; });
    });
    const deliver = vi.fn();
    const notifier = createReplyNotifier("workspace", "session", { plan, deliver });
    const pending = notifier.accept(event(), "en-US");
    notifier.dispose();
    expect(signal.aborted).toBe(true);
    resolve([notification]);
    await pending;
    await notifier.accept(event({ run_id: "next", sequence: 3 }), "en-US");
    expect(deliver).not.toHaveBeenCalled();
    expect(plan).toHaveBeenCalledTimes(1);
  });

  it("cancels a slow request without delivering its late result", async () => {
    vi.useFakeTimers();
    let resolve!: (value: NotificationMessage[]) => void;
    const { notifier, deliver } = fixture(vi.fn(() => new Promise<NotificationMessage[]>((done) => { resolve = done; })));
    const pending = notifier.accept(event(), "en-US");
    await vi.advanceTimersByTimeAsync(5_001);
    resolve([notification]);
    await pending;
    expect(deliver).not.toHaveBeenCalled();
  });

  it("isolates plan failures without falling back to browser policy or retrying duplicates", async () => {
    const { notifier, plan, deliver } = fixture(vi.fn(async () => { throw new Error("policy unavailable"); }));
    await notifier.accept(event(), "en-US");
    await notifier.accept(event({ sequence: 3 }), "en-US");
    expect(plan).toHaveBeenCalledTimes(1);
    expect(deliver).not.toHaveBeenCalled();
  });

  it("retains cancellation for a browser permission request after the plan has completed", async () => {
    const { notifier, deliver } = fixture();
    await notifier.accept(event(), "en-US");
    const signal = deliver.mock.calls[0][1] as AbortSignal;
    expect(signal.aborted).toBe(false);
    notifier.dispose();
    expect(signal.aborted).toBe(true);
  });
});
