import { describe, expect, it } from "vitest";
import { parseSettingsJson } from "./settings-json";

describe("settings JSON validation", () => {
  it("rejects malformed JSON without returning the previous configuration", () => {
    const result = parseSettingsJson('{"providers": [}');
    expect(result.config).toBeNull();
    expect(result.error).toBeTruthy();
  });

  it.each(["null", "[]", "42", "true", '"text"'])("rejects non-object root %s", (text) => {
    expect(parseSettingsJson(text)).toEqual({ config: null, error: "Configuration must be a JSON object" });
  });

  it("preserves unknown fields and secret sentinels for server-side validation", () => {
    const config = { providers: [{ api_key: "saved-secret" }], extension: { custom: [1, false] } };
    expect(parseSettingsJson(JSON.stringify(config))).toEqual({ config, error: null });
  });
});
