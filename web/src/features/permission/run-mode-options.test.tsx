import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { createRunModeOptions } from "./run-mode-options";

describe("createRunModeOptions", () => {
  it("returns the compact permission workflow in display order", () => {
    const options = createRunModeOptions((en) => en);

    expect(options.map(({ value, label }) => ({ value, label }))).toEqual([
      { value: "audited", label: "Confirm changes" },
      { value: "auto_audit", label: "Auto audit" },
      { value: "plan", label: "Plan mode" },
      { value: "yolo", label: "Full access" }
    ]);
    const icons = options.map((option) => renderToStaticMarkup(<>{option.icon}</>));
    expect(icons).toEqual([
      expect.stringContaining("run-mode-icon audit"),
      expect.stringContaining("run-mode-icon auto"),
      expect.stringContaining("run-mode-icon plan"),
      expect.stringContaining("run-mode-icon yolo")
    ]);
    expect(icons[0]).toContain("lucide-hand");
    expect(icons[1]).toContain("lucide-shield-check");
    expect(icons[2]).toContain("lucide-notepad-text");
    expect(icons[3]).toContain("lucide-shield-alert");
  });

  it("uses concise Chinese labels and auto-audit wording", () => {
    const options = createRunModeOptions((_en, zh) => zh);

    expect(options.map(({ label, description }) => ({ label, description }))).toEqual([
      { label: "变更前确认", description: "改文件前先问我。" },
      { label: "自动审核", description: "自动审核文件变更。" },
      { label: "计划模式", description: "编辑前先出计划。" },
      { label: "完全访问", description: "减少确认次数。" }
    ]);
  });
});
