import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { createRunModeOptions } from "./run-mode-options";

describe("createRunModeOptions", () => {
  it("returns the shared yolo audit and plan presentation", () => {
    const options = createRunModeOptions((en) => en);

    expect(options.map(({ value, label }) => ({ value, label }))).toEqual([
      { value: "yolo", label: "YOLO" },
      { value: "audited", label: "Audit" },
      { value: "plan", label: "Plan" }
    ]);
    expect(options.map((option) => renderToStaticMarkup(<>{option.icon}</>))).toEqual([
      expect.stringContaining("run-mode-icon yolo"),
      expect.stringContaining("run-mode-icon audit"),
      expect.stringContaining("run-mode-icon plan")
    ]);
  });
});
