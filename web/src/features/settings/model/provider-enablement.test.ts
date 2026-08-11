import { describe, expect, it } from "vitest";
import type { ProviderConfig } from "../../../api/contracts";
import {
  enabledProviders,
  isProviderEnabled,
  nextActiveProvider,
  partitionByEnablement
} from "./provider-enablement";

/**
 * 构造测试用供应商配置。
 *
 * @param id 供应商标识
 * @param enabled 启用状态；不传表示字段缺省
 * @returns 供应商配置
 */
function provider(id: string, enabled?: boolean): ProviderConfig {
  return {
    id,
    display_name: id,
    base_url: "https://api.example.com/v1",
    ...(enabled === undefined ? {} : { enabled })
  } as ProviderConfig;
}

describe("isProviderEnabled", () => {
  it("treats a missing field as enabled", () => {
    // 旧配置文件没有这一项，默认关闭会让升级后所有供应商一起消失
    expect(isProviderEnabled(provider("a"))).toBe(true);
  });

  it("honours an explicit switch", () => {
    expect(isProviderEnabled(provider("a", true))).toBe(true);
    expect(isProviderEnabled(provider("a", false))).toBe(false);
  });
});

describe("enabledProviders", () => {
  it("drops disabled providers", () => {
    const providers = [provider("a"), provider("b", false), provider("c", true)];

    expect(enabledProviders(providers).map((item) => item.id)).toEqual(["a", "c"]);
  });
});

describe("partitionByEnablement", () => {
  it("splits providers while keeping the original order", () => {
    const providers = [
      provider("a", false),
      provider("b"),
      provider("c", false),
      provider("d", true)
    ];

    const { enabled, disabled } = partitionByEnablement(providers);

    expect(enabled.map((item) => item.id)).toEqual(["b", "d"]);
    expect(disabled.map((item) => item.id)).toEqual(["a", "c"]);
  });
});

describe("nextActiveProvider", () => {
  it("keeps the current provider when it stays enabled", () => {
    const providers = [provider("a"), provider("b", false)];

    expect(nextActiveProvider(providers, "a")).toBe("a");
  });

  it("moves to the first enabled provider when the current one is disabled", () => {
    // 不转移会让后续请求全部落在一个已停用的配置上
    const providers = [provider("a", false), provider("b", false), provider("c")];

    expect(nextActiveProvider(providers, "a")).toBe("c");
  });

  it("returns an empty id when nothing is enabled", () => {
    const providers = [provider("a", false), provider("b", false)];

    expect(nextActiveProvider(providers, "a")).toBe("");
  });

  it("moves away from a provider that no longer exists", () => {
    const providers = [provider("b")];

    expect(nextActiveProvider(providers, "removed")).toBe("b");
  });
});
