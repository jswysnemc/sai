import { describe, expect, it } from "vitest";
import type { BackgroundTask } from "../../api/contracts";
import { backgroundTaskStatusLabel, combineBackgroundTaskOutput, formatBackgroundTaskDuration, sortBackgroundTasks } from "./background-task-utils";

const task = { status: "running", started_at: 100, updated_at: 100 } as BackgroundTask;

describe("background task utils", () => {
  it("格式化运行中和已结束任务时长", () => {
    expect(formatBackgroundTaskDuration(task, 165)).toBe("1分 5秒");
    expect(formatBackgroundTaskDuration({ ...task, status: "exited", updated_at: 3700 }, 9000)).toBe("1小时 0分");
  });

  it("转换状态并合并两种输出流", () => {
    expect(backgroundTaskStatusLabel("timed_out")).toBe("已超时");
    expect(backgroundTaskStatusLabel("stopped")).toBe("已停止");
    expect(backgroundTaskStatusLabel("unknown")).toBe("unknown");
    expect(backgroundTaskStatusLabel("running", "en-US")).toBe("Running");
    expect(combineBackgroundTaskOutput("ready", "failed")).toBe("stdout\nready\n\nstderr\nfailed");
    expect(combineBackgroundTaskOutput(null, "failed")).toBe("stderr\nfailed");
    expect(combineBackgroundTaskOutput()).toBe("");
  });

  it("运行中优先，同组按启动时间倒序排列且不修改原列表", () => {
    const tasks = [
      { ...task, id: "done", status: "exited", started_at: 500 },
      { ...task, id: "old", started_at: 100 },
      { ...task, id: "new", started_at: 300 }
    ];
    expect(sortBackgroundTasks(tasks).map((item) => item.id)).toEqual(["new", "old", "done"]);
    expect(tasks.map((item) => item.id)).toEqual(["done", "old", "new"]);
  });

  it("启动时间在未来时不显示负数时长", () => {
    expect(formatBackgroundTaskDuration(task, 50)).toBe("0秒");
  });
});
