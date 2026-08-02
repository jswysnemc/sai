import { describe, expect, it } from "vitest";
import { detectActivityPulse, type ActivitySnapshot } from "./activity-pulse";

/** 双语翻译桩：测试里固定取中文，便于断言。 */
const t = (en: string, zh: string) => zh;

const base: ActivitySnapshot = {
  runningTasks: 0,
  runningSubagents: 0,
  completedTodos: 0,
  totalTodos: 0
};

describe("detectActivityPulse", () => {
  it("首次快照不播报，避免一进页面就闪一下", () => {
    expect(detectActivityPulse(null, { ...base, runningTasks: 2 }, t)).toBeNull();
  });

  it("后台命令启动时播报", () => {
    const pulse = detectActivityPulse(base, { ...base, runningTasks: 1 }, t);

    expect(pulse?.kind).toBe("task");
    expect(pulse?.message).toContain("1 个后台任务运行中");
  });

  it("后台命令全部结束时播报", () => {
    const pulse = detectActivityPulse({ ...base, runningTasks: 1 }, base, t);

    expect(pulse?.kind).toBe("task");
    expect(pulse?.message).toContain("后台任务已结束");
  });

  it("子智能体优先级高于后台命令", () => {
    const previous = base;
    const current = { ...base, runningTasks: 1, runningSubagents: 1 };

    const pulse = detectActivityPulse(previous, current, t);

    expect(pulse?.kind).toBe("subagent");
  });

  it("Todo 完成数变化时播报计划推进", () => {
    const previous = { ...base, totalTodos: 3, completedTodos: 1 };
    const current = { ...base, totalTodos: 3, completedTodos: 2 };

    const pulse = detectActivityPulse(previous, current, t);

    expect(pulse?.kind).toBe("todo");
    expect(pulse?.message).toContain("2/3");
  });

  it("数量未变化时不播报", () => {
    const snapshot = { ...base, runningTasks: 1, totalTodos: 2, completedTodos: 1 };

    expect(detectActivityPulse(snapshot, snapshot, t)).toBeNull();
  });
});
