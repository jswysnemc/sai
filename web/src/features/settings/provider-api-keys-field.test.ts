import { describe, expect, it } from "vitest";
import { editorProviderApiKeys } from "./provider-api-keys-field";

describe("editorProviderApiKeys", () => {
  it("gives an empty key row when the provider has none", () => {
    expect(editorProviderApiKeys([])).toEqual([{ id: "key-1", api_key: "", label: "" }]);
  });

  it("keeps existing keys unchanged", () => {
    const keys = [{ id: "key-2", api_key: "sk-test", label: "work" }];
    expect(editorProviderApiKeys(keys)).toBe(keys);
  });
});
