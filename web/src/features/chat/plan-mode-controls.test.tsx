import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { parsePlanCommand, PlanModeControls } from "./plan-mode-controls";
import { createRunModeOptions } from "../permission/run-mode-options";

describe("independent Plan mode", () => {
  it("recognizes the plan command without matching unrelated paths or skills", () => {
    expect(parsePlanCommand("/plan")).toBe("");
    expect(parsePlanCommand(" /PLAN design an API\ninclude tests ")).toBe("design an API\ninclude tests");
    expect(parsePlanCommand("/planner")).toBeNull();
    expect(parsePlanCommand("see /plan")).toBeNull();
  });
  it("keeps Plan outside permission options and shows it as a separate active control", () => {
    expect(createRunModeOptions((en) => en).map((item) => item.value)).toEqual(["audited", "auto_audit", "yolo"]);
    const html = renderToStaticMarkup(<PlanModeControls mode="plan" disabled={false} onEnter={() => {}} onLeave={() => {}} />);
    expect(html).toContain('aria-pressed="true"');
    expect(html).toContain("退出规划，不执行计划");
  });
});
