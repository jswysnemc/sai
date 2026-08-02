import { describe, expect, it } from "vitest";
import { parsePickedDirectory, pickedDirectoryName, resolveDirectoryCandidates } from "./picked-directory";

describe("picked-directory", () => {
  it("从文件相对路径取出被选目录名", () => {
    expect(pickedDirectoryName(["sandbox/src/main.rs", "sandbox/Cargo.toml"])).toBe("sandbox");
  });

  it("忽略空路径，取第一个可用首段", () => {
    expect(pickedDirectoryName(["", "/", "web/index.html"])).toBe("web");
  });

  it("空选择返回空串", () => {
    expect(pickedDirectoryName([])).toBe("");
  });

  it("把目录名拼到每个允许根之下", () => {
    expect(resolveDirectoryCandidates("sandbox", ["/home/snemc", "/srv/work"]))
      .toEqual(["/home/snemc/sandbox", "/srv/work/sandbox"]);
  });

  it("根路径末尾多余的斜杠不会产生双斜杠", () => {
    expect(resolveDirectoryCandidates("sandbox", ["/home/snemc/"]))
      .toEqual(["/home/snemc/sandbox"]);
  });

  it("目录名为空时没有候选", () => {
    expect(resolveDirectoryCandidates("", ["/home/snemc"])).toEqual([]);
    expect(parsePickedDirectory([], ["/home/snemc"])).toEqual({ name: "", candidates: [] });
  });
});
