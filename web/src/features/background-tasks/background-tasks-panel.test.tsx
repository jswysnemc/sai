import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BackgroundTask } from "../../api/contracts";
import { DialogProvider } from "../../shared/ui/dialog/dialog-provider";
import { BackgroundTasksPanel } from "./background-tasks-panel";
import { BackgroundTaskLogView } from "./background-task-log-view";
import { useBackgroundTasks } from "./use-background-tasks";

vi.mock("./use-background-tasks", () => ({ useBackgroundTasks: vi.fn() }));

const task: BackgroundTask = {
  id: "build", label: "Build", command: "pnpm build", cwd: "/project", pid: 42,
  status: "running", started_at: 100, updated_at: 110, timeout_seconds: 600
};

/**
 * 创建默认任务管理状态，可覆盖需要验证的查询或操作状态。
 * @param overrides 需要覆盖的状态
 * @returns 渲染后的面板标记
 */
function renderPanel(overrides: Partial<ReturnType<typeof useBackgroundTasks>> = {}) {
  vi.mocked(useBackgroundTasks).mockReturnValue({
    tasks: [task], selected: task, output: undefined,
    loading: false, outputLoading: false, error: null, outputError: null,
    refreshing: false, stoppingId: null, cleaning: false,
    stop: vi.fn(), cleanup: vi.fn(), refresh: vi.fn(), ...overrides,
    visibleTasks: overrides.visibleTasks ?? overrides.tasks ?? [task]
  });
  return renderToStaticMarkup(<DialogProvider><BackgroundTasksPanel /></DialogProvider>);
}

beforeEach(() => vi.clearAllMocks());

describe("后台任务面板展示", () => {
  it("使用统一按钮并为当前任务标明选中状态", () => {
    const html = renderPanel();
    expect(html).toMatch(/class="ui-button[^\"]*background-task-select"[^>]*aria-pressed="true"/);
    expect(html).toContain('aria-label="停止任务"');
    expect(html).toContain('aria-label="任务详情"');
    expect(html).toContain(">42</dd>");
    expect(html).toContain("/project");
    expect(html).toContain("暂无输出");
  });

  it("区分加载与获取失败，不将错误显示为空列表", () => {
    const loading = renderPanel({ tasks: [], selected: undefined, loading: true });
    expect(loading).toContain("正在获取任务");
    expect(loading).not.toContain("没有后台任务");
    const failed = renderPanel({ tasks: [], selected: undefined, error: new Error("offline") });
    expect(failed).toContain('role="alert"');
    expect(failed).toContain("获取任务失败：offline");
    expect(failed).not.toContain("没有后台任务");
  });

  it("没有已结束任务时禁用清理，停止请求期间禁用重复操作", () => {
    const html = renderPanel({ stoppingId: task.id });
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*aria-label="清理已结束任务"/);
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*aria-busy="true"[^>]*aria-label="停止任务"/);
    expect(html).toContain("正在停止");
  });

  it("保留已停止和超时状态，不统一显示为正常结束", () => {
    const html = renderPanel({ tasks: [
      { ...task, id: "stopped", status: "stopped" },
      { ...task, id: "timeout", status: "timed_out" }
    ], selected: { ...task, id: "stopped", status: "stopped" } });
    expect(html).toContain("已停止");
    expect(html).toContain("已超时");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*aria-label="停止任务"/);
  });

  it("展示输出截断提示及错误，保留已获得的输出", () => {
    const html = renderToStaticMarkup(<BackgroundTaskLogView task={task}
      output={{ task, stdout: "build output", stderr: "build error", stdout_truncated: true, stderr_truncated: false, tail_lines: 200 }}
      error={new Error("read failed")} loading={false} stream="all" onStreamChange={vi.fn()} onBack={vi.fn()} />);
    expect(html).toContain("末尾 200 行");
    expect(html).toContain("读取输出失败：read failed");
    expect(html).toContain("build output");
    expect(html).toContain("build error");
    expect(html).toContain("按通道分组，非时间顺序");
    expect(html).not.toContain("暂无输出");
  });

  it("紧凑列表包含筛选和日志入口，未进入日志模式时只显示摘要", () => {
    const html = renderPanel({ output: { task, stdout: "first\nlast", stderr: "", stdout_truncated: false, stderr_truncated: false, tail_lines: 2000 } });
    expect(html).toContain('aria-label="任务筛选"');
    expect(html).toContain("查看日志");
    expect(html).toContain("强制停止");
    expect(html).toContain("last");
    expect(html).not.toContain("first");
    expect(html).not.toContain("background-task-log-viewport");
  });

  it("读取输出时显示加载状态，首次读取失败时不显示空输出", () => {
    expect(renderPanel({ outputLoading: true })).toContain("正在读取输出");
    const failed = renderPanel({ outputError: new Error("read failed") });
    expect(failed).toContain("读取输出失败");
    expect(failed).not.toContain("暂无输出");
  });
});
