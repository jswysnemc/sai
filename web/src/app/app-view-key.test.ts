import { describe, expect, it } from "vitest";
import { appViewKey } from "./app-view-key";

describe("appViewKey", () => {
  it("keeps the same key across settings sections and subviews", () => {
    // 分区与子页切换是页内导航，重挂载会丢掉设置页的编辑草稿
    const connection = appViewKey("/settings/providers/connection");
    expect(appViewKey("/settings/providers/behavior")).toBe(connection);
    expect(appViewKey("/settings/providers/models")).toBe(connection);
    expect(appViewKey("/settings/git")).toBe(connection);
    expect(appViewKey("/settings")).toBe(connection);
  });

  it("changes the key between top-level pages", () => {
    const settings = appViewKey("/settings/providers/connection");
    expect(appViewKey("/gateways")).not.toBe(settings);
    expect(appViewKey("/cron-jobs")).not.toBe(settings);
    expect(appViewKey("/")).not.toBe(settings);
  });

  it("treats the root path as a stable key", () => {
    expect(appViewKey("/")).toBe("");
    expect(appViewKey("")).toBe("");
  });

  it("ignores repeated separators", () => {
    expect(appViewKey("//settings//providers")).toBe("settings");
  });
});
