import { describe, expect, it } from "vitest";
import type { BackgroundTaskOutput } from "../../api/contracts";
import { backgroundTaskLogLines, backgroundTaskLogTruncated, cleanBackgroundTaskLog } from "./background-task-log-model";
import { filterBackgroundTasks } from "./background-task-utils";

const output = { stdout: "one\ntwo\n", stderr: "error\n", stdout_truncated: true, stderr_truncated: false } as BackgroundTaskOutput;

describe("后台任务日志模型", () => {
  it("通道独立编号，全部模式按通道分组而非交错伪造时间顺序", () => {
    expect(backgroundTaskLogLines(output, "all")).toEqual([
      { stream: "stdout", number: 1, text: "one" }, { stream: "stdout", number: 2, text: "two" },
      { stream: "stderr", number: 1, text: "error" }
    ]);
    expect(backgroundTaskLogLines(output, "stderr")).toHaveLength(1);
    expect(backgroundTaskLogLines(undefined, "stdout")).toEqual([]);
  });
  it("只对发生截断的当前通道提示截断", () => {
    expect(backgroundTaskLogTruncated(output, "stdout")).toBe(true);
    expect(backgroundTaskLogTruncated(output, "stderr")).toBe(false);
    expect(backgroundTaskLogTruncated(output, "all")).toBe(true);
  });
  it("清除样式和终端控制序列，保留内容", () => {
    expect(cleanBackgroundTaskLog("\x1b[31merror\x1b[0m\x1b]52;c;secret\x07")).toBe("error");
    expect(backgroundTaskLogLines({ ...output, stdout: "a\r\nb\r" }, "stdout").map((line) => line.text)).toEqual(["a", "b"]);
  });
  it("已结束范围包含停止、失败、超时和正常退出，不包含运行任务", () => {
    const tasks = ["running", "exited", "stopped", "timed_out", "failed"].map((status) => ({ ...output.task, status }));
    expect(filterBackgroundTasks(tasks, "finished").map((task) => task.status)).toEqual(["exited", "stopped", "timed_out", "failed"]);
  });
});
