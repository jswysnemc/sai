import { describe, expect, it } from "vitest";
import { parseGoalCommand } from "./goal-command";

describe("parseGoalCommand", () => {
  it("parses plain goal commands", () => {
    expect(parseGoalCommand("/goal 完成 Git 管理功能")).toEqual({ objective: "完成 Git 管理功能" });
    expect(parseGoalCommand(" /goal 第一行\n第二行 ")).toEqual({ objective: "第一行\n第二行" });
    expect(parseGoalCommand("/goal")).toEqual({ objective: "" });
  });

  it("accepts skill-mention and fullwidth slash forms", () => {
    expect(parseGoalCommand('<skill-mention name="goal"></skill-mention> 完成重构')).toEqual({
      objective: "完成重构"
    });
    expect(parseGoalCommand("／goal 完成重构")).toEqual({ objective: "完成重构" });
    expect(parseGoalCommand("/Goal 完成重构")).toEqual({ objective: "完成重构" });
  });

  it("accepts Chinese objective without space after /goal", () => {
    expect(parseGoalCommand("/goal完成功能")).toEqual({ objective: "完成功能" });
  });

  it("does not treat ordinary text or other skills as goal commands", () => {
    expect(parseGoalCommand("说明 /goal 的用法")).toBeNull();
    expect(parseGoalCommand("/goalie fix")).toBeNull();
    expect(parseGoalCommand("完成功能")).toBeNull();
  });
});
