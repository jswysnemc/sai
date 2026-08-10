import { describe, expect, it } from "vitest";
import type { ProviderConfig } from "../../../api/contracts";
import { findProviderIndex, nextProviderId, providerIdConflict } from "./provider-selection";

/**
 * 构造测试用供应商配置。
 *
 * @param id 供应商标识
 * @returns 供应商配置
 */
function provider(id: string): ProviderConfig {
  return {
    id,
    display_name: id,
    base_url: "https://api.example.com/v1",
    protocol: "auto",
    api_key: "",
    models: [],
    default_model: ""
  } as ProviderConfig;
}

describe("findProviderIndex", () => {
  it("locates an existing provider", () => {
    const providers = [provider("a"), provider("b"), provider("c")];
    expect(findProviderIndex(providers, "b")).toBe(1);
  });

  it("returns -1 instead of falling back to the first provider", () => {
    // 回落到 0 会让按索引写入打到别的供应商，表现为自动切换并丢失编辑内容
    const providers = [provider("a"), provider("b")];
    expect(findProviderIndex(providers, "missing")).toBe(-1);
    expect(findProviderIndex(providers, "")).toBe(-1);
  });

  it("returns -1 for an empty provider list", () => {
    expect(findProviderIndex([], "a")).toBe(-1);
  });
});

describe("nextProviderId", () => {
  it("uses the bare name when it is free", () => {
    expect(nextProviderId([])).toBe("provider");
  });

  it("skips ids that are already taken", () => {
    const providers = [provider("provider"), provider("provider-2")];
    expect(nextProviderId(providers)).toBe("provider-3");
  });
});

describe("providerIdConflict", () => {
  it("accepts a free id", () => {
    const providers = [provider("a"), provider("b")];
    expect(providerIdConflict(providers, 0, "c")).toBe("");
  });

  it("accepts keeping the current id", () => {
    const providers = [provider("a"), provider("b")];
    expect(providerIdConflict(providers, 0, "a")).toBe("");
  });

  it("rejects an empty id", () => {
    const providers = [provider("a")];
    expect(providerIdConflict(providers, 0, "   ")).toBe("empty");
  });

  it("rejects an id used by another provider", () => {
    const providers = [provider("a"), provider("b")];
    expect(providerIdConflict(providers, 0, "b")).toBe("duplicate");
  });
});
