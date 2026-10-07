import { describe, expect, it } from "vitest";
import { turnFileDisplayParts } from "./turn-file-changes";

describe("turnFileDisplayParts", () => {
  it("工作区内绝对路径只展示相对目录", () => {
    expect(turnFileDisplayParts(
      "/home/snemc/workspace/demo/android-helper/src/MemoryWorker.java",
      "/home/snemc/workspace/demo"
    )).toEqual({
      name: "MemoryWorker.java",
      directory: "android-helper/src"
    });
  });

  it("已经是工作区相对路径时原样拆分", () => {
    expect(turnFileDisplayParts(
      "crates/server/tests/percentage_only_load.rs",
      "/home/snemc/workspace/demo"
    )).toEqual({
      name: "percentage_only_load.rs",
      directory: "crates/server/tests"
    });
  });

  it("工作区外路径保留完整目录", () => {
    expect(turnFileDisplayParts("/tmp/outside/notes.md", "/home/snemc/workspace/demo")).toEqual({
      name: "notes.md",
      directory: "/tmp/outside"
    });
  });
});
