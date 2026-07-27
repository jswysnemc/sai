import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { initialRunState, type LiveRunState } from "../run-event-reducer";
import { QueuedMessageList, reorderQueuedRuns } from "./queued-message-list";

/**
 * 构造队列组件测试使用的排队运行。
 *
 * @param id 运行标识
 * @param input 消息正文
 * @returns 排队运行状态
 */
function queuedRun(id: string, input: string): LiveRunState {
  return {
    ...initialRunState,
    runId: id,
    sessionId: "session",
    status: "queued",
    userInput: input
  };
}

describe("QueuedMessageList", () => {
  it("renders queued messages as compact rows with all expected actions", () => {
    const html = renderToStaticMarkup(
      <QueuedMessageList
        runs={[queuedRun("run-1", "first"), queuedRun("run-2", "second")]}
        onUpdate={vi.fn()}
        onMove={vi.fn()}
        onRemove={vi.fn()}
        onError={vi.fn()}
      />
    );

    expect(html).toContain('class="queued-message-list"');
    expect(html).toContain("first");
    expect(html).toContain("second");
    expect(html).toContain("当前任务结束后立即执行");
    expect(html).toContain("编辑排队消息");
    expect(html).toContain("删除排队消息");
    expect(html).not.toContain("user-message is-queued");
  });

  it("moves a queued message to the requested position without mutating input", () => {
    const runs = [queuedRun("run-1", "first"), queuedRun("run-2", "second"), queuedRun("run-3", "third")];
    const reordered = reorderQueuedRuns(runs, "run-3", 0);

    expect(reordered.map((run) => run.runId)).toEqual(["run-3", "run-1", "run-2"]);
    expect(runs.map((run) => run.runId)).toEqual(["run-1", "run-2", "run-3"]);
  });

  it("clamps keyboard and drag destinations to the queue bounds", () => {
    const runs = [queuedRun("run-1", "first"), queuedRun("run-2", "second")];

    expect(reorderQueuedRuns(runs, "run-1", 99).map((run) => run.runId)).toEqual(["run-2", "run-1"]);
    expect(reorderQueuedRuns(runs, "run-2", -3).map((run) => run.runId)).toEqual(["run-2", "run-1"]);
  });
});
