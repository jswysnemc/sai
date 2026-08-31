import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { DialogProvider } from "../../../shared/ui/dialog/dialog-provider";
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

const queueNoops = {
  onUpdate: vi.fn(),
  onMove: vi.fn(),
  onPromote: vi.fn(),
  onInsertAt: vi.fn(),
  onRemove: vi.fn(),
  onError: vi.fn()
};

/**
 * 在确认框上下文中渲染队列，供静态 HTML 断言。
 *
 * @param runs 排队运行
 * @returns 静态 HTML
 */
function renderQueue(runs: LiveRunState[]): string {
  return renderToStaticMarkup(
    <DialogProvider>
      <QueuedMessageList runs={runs} {...queueNoops} />
    </DialogProvider>
  );
}

describe("QueuedMessageList", () => {
  it("renders queued messages as compact rows with all expected actions", () => {
    const html = renderQueue([queuedRun("run-1", "first"), queuedRun("run-2", "second")]);

    expect(html).toContain('class="queued-message-list"');
    expect(html).toContain("first");
    expect(html).toContain("second");
    expect(html).toContain("下次模型请求时优先插入");
    expect(html).toContain("编辑排队消息");
    expect(html).toContain("删除排队消息");
    // 队列行渲染为紧凑预览而非聊天气泡：气泡的 fit-content 宽度与投影不适合并排成列表
    expect(html).toContain('class="queued-message-preview"');
    expect(html).not.toContain("user-bubble");
  });

  it("numbers each row and surfaces the queue length", () => {
    const html = renderQueue([queuedRun("run-1", "first"), queuedRun("run-2", "second")]);

    expect(html).toContain("2 条待发送");
    expect(html).toContain('class="queued-message-index" aria-hidden="true">1<');
    expect(html).toContain('class="queued-message-index" aria-hidden="true">2<');
  });

  it("marks image-only messages instead of rendering an empty row", () => {
    const html = renderQueue([{ ...queuedRun("run-1", "  "), imageUrls: ["/a.png", "/b.png"] }]);

    expect(html).toContain("仅图片");
    expect(html).toContain("附带 2 张图片");
    expect(html).toContain('class="queued-message-thumbs"');
    expect(html).toContain('src="/a.png"');
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
