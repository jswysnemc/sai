import { describe, expect, it } from "vitest";
import { parseRenameCommand } from "./rename-command";

describe("parseRenameCommand", () => {
  it("reads a title after /rename", () => {
    expect(parseRenameCommand("/rename Sprint plan")).toEqual({ title: "Sprint plan" });
    expect(parseRenameCommand(" /rename 第一行\n第二行 ")).toEqual({ title: "第一行\n第二行" });
    expect(parseRenameCommand("/rename")).toEqual({ title: "" });
  });

  it("accepts the Chinese alias and fullwidth slash", () => {
    expect(parseRenameCommand("/重命名 本周计划")).toEqual({ title: "本周计划" });
    expect(parseRenameCommand("／rename 本周计划")).toEqual({ title: "本周计划" });
    expect(parseRenameCommand("/Rename Demo")).toEqual({ title: "Demo" });
  });

  it("accepts a Chinese title glued to the command", () => {
    expect(parseRenameCommand("/rename本周计划")).toEqual({ title: "本周计划" });
    expect(parseRenameCommand("/重命名本周计划")).toEqual({ title: "本周计划" });
  });

  it("ignores ordinary text and similar command names", () => {
    expect(parseRenameCommand("说明 /rename 的用法")).toBeNull();
    expect(parseRenameCommand("/renamed leftover")).toBeNull();
    expect(parseRenameCommand("本周计划")).toBeNull();
  });
});
