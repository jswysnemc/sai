import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BackgroundTask } from "../../api/contracts";
import { useBackgroundTaskActions } from "./use-background-task-actions";

const { confirm, showToast, dismissToast } = vi.hoisted(() => ({ confirm: vi.fn(), showToast: vi.fn(), dismissToast: vi.fn() }));
vi.mock("../../shared/ui/dialog/dialog-provider", () => ({ useConfirm: () => confirm }));
vi.mock("../../shared/ui/notify/notify", () => ({ useToast: () => ({ notice: null, showToast, dismissToast }) }));

const task = { id: "task", label: "Build", command: "pnpm build" } as BackgroundTask;

beforeEach(() => {
  vi.resetAllMocks();
  confirm.mockResolvedValue(true);
});

/**
 * 创建受控任务操作和真实确认流程。
 * @returns 管理接口替身以及捕获错误的操作方法
 */
function setup() {
  const manager = {
    stop: vi.fn().mockResolvedValue({ task, was_running: true }),
    cleanup: vi.fn().mockResolvedValue({ removed: ["task"], remaining: 0 }),
    refresh: vi.fn().mockResolvedValue(undefined)
  };
  let actions!: ReturnType<typeof useBackgroundTaskActions>;
  /** 读取操作方法，返回空渲染结果。 */
  function Probe() {
    actions = useBackgroundTaskActions(manager);
    return null;
  }
  renderToStaticMarkup(<Probe />);
  return { manager, actions };
}

describe("后台任务操作确认", () => {
  it("取消停止或清理时不发送管理请求", async () => {
    confirm.mockResolvedValue(false);
    const { manager, actions } = setup();
    await actions.stop(task);
    await actions.cleanup();
    expect(manager.stop).not.toHaveBeenCalled();
    expect(manager.cleanup).not.toHaveBeenCalled();
    expect(showToast).not.toHaveBeenCalled();
  });

  it("停止前显示任务名与命令，完成后显示成功提示", async () => {
    const { manager, actions } = setup();
    await actions.stop(task);
    expect(confirm).toHaveBeenCalledWith(expect.objectContaining({ danger: true, description: expect.stringContaining("Build") }));
    expect(confirm.mock.calls[0][0].description).toContain("pnpm build");
    expect(manager.stop).toHaveBeenCalledWith("task");
    expect(showToast).toHaveBeenCalledWith("任务已停止");
  });

  it("强制停止有独立确认，并将强制参数交给真实操作", async () => {
    const { manager, actions } = setup();
    await actions.stop(task, true);
    expect(confirm.mock.calls[0][0].title).toBe("强制停止后台任务？");
    expect(confirm.mock.calls[0][0].description).toContain("没有宽限期");
    expect(manager.stop).toHaveBeenCalledWith({ id: "task", force: true });
    expect(showToast).toHaveBeenCalledWith("任务已强制停止");
  });

  it("操作失败使用错误提示，后续操作仍可执行", async () => {
    const { manager, actions } = setup();
    manager.stop.mockRejectedValueOnce(new Error("permission denied"));
    await expect(actions.stop(task)).resolves.toBeUndefined();
    expect(showToast).toHaveBeenCalledWith("permission denied", "error");
    await actions.cleanup();
    expect(showToast).toHaveBeenCalledWith("已清理 1 条任务记录");
    expect(confirm.mock.calls[1][0].description).toContain("保留运行中的任务和日志文件");
  });

  it("确认尚未结束时忽略重复操作", async () => {
    let resolve!: (accepted: boolean) => void;
    confirm.mockImplementationOnce(() => new Promise<boolean>((accept) => { resolve = accept; }));
    const { manager, actions } = setup();
    const pending = actions.stop(task);
    await actions.stop(task);
    await actions.cleanup();
    expect(confirm).toHaveBeenCalledTimes(1);
    resolve(true);
    await pending;
    expect(manager.stop).toHaveBeenCalledTimes(1);
    expect(manager.cleanup).not.toHaveBeenCalled();
  });

  it("刷新失败被捕获，已自然结束的任务不误报为停止成功", async () => {
    const { manager, actions } = setup();
    manager.refresh.mockRejectedValueOnce(new Error("offline"));
    await actions.refresh();
    expect(showToast).toHaveBeenCalledWith("offline", "error");
    manager.stop.mockResolvedValueOnce({ task, was_running: false });
    await actions.stop(task);
    expect(showToast).toHaveBeenCalledWith("任务已经结束");
  });
});
