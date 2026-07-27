import { describe, expect, it } from "vitest";
import { parseAcpRuntimeOptions } from "./acp-runtime-config-fields";

describe("ACP runtime config fields", () => {
  it("parses grouped select options and booleans", () => {
    const options = parseAcpRuntimeOptions([
      {
        id: "model",
        name: "Model",
        category: "model",
        type: "select",
        currentValue: "sonnet",
        options: [{ name: "Claude", options: [{ value: "sonnet", name: "Sonnet" }] }]
      },
      { id: "tools", name: "Tools", type: "boolean", currentValue: true }
    ]);

    expect(options[0]?.values).toEqual([{ value: "sonnet", label: "Sonnet" }]);
    expect(options[1]?.currentValue).toBe(true);
  });

  it("ignores unsupported option shapes", () => {
    expect(parseAcpRuntimeOptions([{ id: "count", name: "Count", type: "number" }])).toEqual([]);
  });
});
