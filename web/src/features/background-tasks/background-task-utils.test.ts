import { describe, expect, it } from "vitest";
import type { BackgroundTask } from "../../api/contracts";
import { backgroundTaskStatusLabel, combineBackgroundTaskOutput, formatBackgroundTaskDuration } from "./background-task-utils";

const task = { status: "running", started_at: 100, updated_at: 100 } as BackgroundTask;

describe("background task utils", () => {
  it("格式化运行中和已结束任务时长", () => {
    expect(formatBackgroundTaskDuration(task, 165)).toBe("1分 5秒");
    expect(formatBackgroundTaskDuration({ ...task, status: "exited", updated_at: 3700 }, 9000)).toBe("1小时 0分");
  });

  it("转换状态并合并两种输出流", () => {
    expect(backgroundTaskStatusLabel("timed_out")).toBe("已超时");
    expect(combineBackgroundTaskOutput("ready", "failed")).toBe("stdout\nready\n\nstderr\nfailed");
  });
});
