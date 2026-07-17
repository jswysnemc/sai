import { describe, expect, it } from "vitest";
import type { CronJob } from "../../api/contracts";
import { formatCronInterval, getCronJobStatus } from "./cron-job-utils";

/** 创建用于状态测试的定时任务。 */
function job(overrides: Partial<CronJob> = {}): CronJob {
  return {
    id: "cron_1",
    name: "测试任务",
    prompt: "执行测试",
    session_id: "session_1",
    interval_seconds: null,
    next_run_at: 1,
    enabled: true,
    failure_count: 0,
    last_error: null,
    ...overrides
  };
}

describe("formatCronInterval", () => {
  it("区分单次、分钟、小时和天级间隔", () => {
    expect(formatCronInterval(null)).toBe("单次执行");
    expect(formatCronInterval(300)).toBe("每 5 分钟");
    expect(formatCronInterval(7_200)).toBe("每 2 小时");
    expect(formatCronInterval(172_800)).toBe("每 2 天");
  });
});

describe("getCronJobStatus", () => {
  it("显示启用、手动停用和失败停用状态", () => {
    expect(getCronJobStatus(job())).toBe("已启用");
    expect(getCronJobStatus(job({ enabled: false }))).toBe("已停用");
    expect(getCronJobStatus(job({ enabled: false, failure_count: 3 }))).toBe("失败后停用");
  });
});
