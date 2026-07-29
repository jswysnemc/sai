import { describe, expect, it } from "vitest";
import {
  classifyCliToolField,
  groupCliToolFields,
  isCliToolEnabledField
} from "./cli-tool-field-groups";

describe("CLI tool field groups", () => {
  it("将总开关从普通字段组中排除", () => {
    const groups = groupCliToolFields({
      enabled: true,
      api_keys: ["key"],
      base_url: "https://example.test",
      timeout_seconds: 20,
      safe_search: true,
      model: "test"
    });

    expect(groups.map((group) => group.id)).toEqual([
      "credentials",
      "endpoints",
      "limits",
      "switches",
      "other"
    ]);
    expect(groups.flatMap((group) => group.entries).some(([name]) => name === "enabled")).toBe(false);
  });

  it("按字段名称和类型识别分组", () => {
    expect(classifyCliToolField("api_key", "")).toBe("credentials");
    expect(classifyCliToolField("output_dir", "")).toBe("endpoints");
    expect(classifyCliToolField("max_rounds", 2)).toBe("limits");
    expect(classifyCliToolField("preview", true)).toBe("switches");
    expect(classifyCliToolField("model", "")).toBe("other");
    expect(isCliToolEnabledField("enabled")).toBe(true);
  });
});
