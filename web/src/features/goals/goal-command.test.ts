import { describe, expect, it } from "vitest";
import { parseGoalCommand } from "./goal-command";

describe("parseGoalCommand", () => {
  it("extracts single and multiline objectives", () => {
    expect(parseGoalCommand("/goal 完成 Git 管理功能")).toEqual({ objective: "完成 Git 管理功能" });
    expect(parseGoalCommand(" /goal 第一行\n第二行 ")).toEqual({ objective: "第一行\n第二行" });
  });

  it("distinguishes an empty goal command from normal input", () => {
    expect(parseGoalCommand("/goal")).toEqual({ objective: "" });
    expect(parseGoalCommand("说明 /goal 的用法")).toBeNull();
  });
});
