import { describe, expect, it } from "vitest";
import type { LiveMessagePart } from "../run-event-reducer";
import { collectWaveSecrets, collectWaveTools, countWorkItems, groupActivityParts, splitWaveForEarlyReadFold } from "./group-activity-parts";
import type { ToolLifecycle } from "../run-event-reducer";

function tool(id: string, name = "read_file"): LiveMessagePart {
  const lifecycle: ToolLifecycle = {
    id,
    name,
    argumentsPreview: "",
    arguments: "{}",
    progress: "",
    output: "",
    status: "completed"
  };
  return { id, type: "tool", tool: lifecycle };
}

function reasoning(id: string): LiveMessagePart {
  return { id, type: "reasoning", source: "think", startedAt: "" };
}

function text(id: string): LiveMessagePart {
  return { id, type: "text", source: "hello" };
}

function sshSecret(id: string): LiveMessagePart {
  return {
    id,
    type: "ssh_secret",
    request: {
      id: `${id}-req`,
      session_id: "s1",
      kind: "password",
      host_label: "local",
      prompt: "未配置私钥，请输入该主机的登录密码。",
      changed: false
    }
  };
}

describe("groupActivityParts", () => {
  it("folds consecutive reasoning and tools before assistant text", () => {
    const segments = groupActivityParts([
      reasoning("r1"),
      tool("t1"),
      tool("t2"),
      text("body")
    ]);

    expect(segments).toHaveLength(2);
    expect(segments[0]).toMatchObject({ type: "preamble", followedByText: true });
    if (segments[0].type !== "preamble") throw new Error("expected preamble");
    expect(segments[0].items).toHaveLength(2);
    expect(segments[0].items[0]).toMatchObject({ kind: "reasoning" });
    expect(segments[0].items[1]).toMatchObject({ kind: "wave" });
    if (segments[0].items[1].kind !== "wave") throw new Error("expected wave");
    expect(segments[0].items[1].parts).toHaveLength(2);
    expect(segments[1]).toMatchObject({ type: "part" });
  });

  it("splits a new work group after assistant text", () => {
    const segments = groupActivityParts([
      reasoning("r1"),
      tool("t1"),
      text("body"),
      reasoning("r2"),
      tool("t2")
    ]);

    expect(segments.map((segment) => segment.type)).toEqual(["preamble", "part", "preamble"]);
    if (segments[2].type !== "preamble") throw new Error("expected trailing preamble");
    expect(segments[2].followedByText).toBe(false);
  });

  it("keeps a reasoning break between two tool waves", () => {
    const segments = groupActivityParts([
      tool("t1"),
      tool("t2"),
      reasoning("r1"),
      tool("t3")
    ]);

    if (segments[0].type !== "preamble") throw new Error("expected preamble");
    expect(segments[0].items.map((item) => item.kind)).toEqual(["wave", "reasoning", "wave"]);
  });

  it("keeps an SSH password card inside the tool wave", () => {
    const segments = groupActivityParts([
      tool("t1"),
      sshSecret("sec1"),
      text("body")
    ]);

    expect(segments).toHaveLength(2);
    if (segments[0].type !== "preamble") throw new Error("expected preamble");
    expect(segments[0].items).toHaveLength(1);
    if (segments[0].items[0].kind !== "wave") throw new Error("expected wave");
    expect(segments[0].items[0].parts.map((part) => part.type)).toEqual(["tool", "ssh_secret"]);
    expect(collectWaveSecrets(segments[0].items).map((part) => part.id)).toEqual(["sec1"]);
  });

  it("keeps the pre-send Jev judgment before the tool overview", () => {
    const segments = groupActivityParts([
      { id: "jev", type: "jev", phase: "ready", exposure: { tools: [], skills: [], contexts: [] }, detail: "" },
      tool("t1"),
      text("body")
    ]);
    expect(segments.map((segment) => segment.type)).toEqual(["part", "preamble", "part"]);
    expect(segments[0]).toMatchObject({ type: "part", part: { type: "jev" } });
  });

  it("counts reasoning segments and tools", () => {
    const segments = groupActivityParts([reasoning("r1"), tool("t1"), tool("t2")]);
    if (segments[0].type !== "preamble") throw new Error("expected preamble");
    expect(countWorkItems(segments[0].items)).toEqual({
      reasoning: 1,
      tools: 2,
      read: 2,
      write: 0,
      command: 0,
      other: 0
    });
    expect(collectWaveTools(segments[0].items).map((part) => part.id)).toEqual(["t1", "t2"]);
  });

  it("按用途拆分工具计数", () => {
    const segments = groupActivityParts([
      reasoning("r1"),
      tool("t1", "read_file"),
      tool("t2", "grep"),
      tool("t3", "write_file"),
      tool("t4", "str_replace"),
      tool("t5", "run_command"),
      tool("t6", "subagent")
    ]);
    if (segments[0].type !== "preamble") throw new Error("expected preamble");
    expect(countWorkItems(segments[0].items)).toEqual({
      reasoning: 1,
      tools: 6,
      read: 2,
      write: 2,
      command: 1,
      other: 1
    });
  });

  it("background_command 视为命令类", () => {
    const segments = groupActivityParts([
      tool("t1", "background_command")
    ]);
    if (segments[0].type !== "preamble") throw new Error("expected preamble");
    expect(countWorkItems(segments[0].items).command).toBe(1);
  });
});

describe("blank text between work rounds", () => {
  const blank = (id: string): LiveMessagePart => ({ id, type: "text", source: "\n\n" });

  it("空白正文不拆开连续的思考与工具", () => {
    const segments = groupActivityParts([
      reasoning("r1"), tool("t1"), blank("b1"), reasoning("r2"), tool("t2"), blank("b2"), reasoning("r3"), text("final")
    ]);
    expect(segments.map((segment) => segment.type)).toEqual(["preamble", "part"]);
    const preamble = segments[0];
    expect(preamble.type === "preamble" && countWorkItems(preamble.items)).toEqual({
      reasoning: 3,
      tools: 2,
      read: 2,
      write: 0,
      command: 0,
      other: 0
    });
    expect(preamble.type === "preamble" && preamble.followedByText).toBe(true);
  });

  it("末尾的空白正文不算作后接正文", () => {
    const segments = groupActivityParts([reasoning("r1"), tool("t1"), blank("b1")]);
    expect(segments).toHaveLength(1);
    expect(segments[0].type === "preamble" && segments[0].followedByText).toBe(false);
  });
});

describe("splitWaveForEarlyReadFold", () => {
  /**
   * 构造测试用的工具部件。
   *
   * @param id 部件标识
   * @param name 工具名
   * @param status 生命周期状态
   * @returns 工具部件
   */
  function toolWavePart(id: string, name: string, status: ToolLifecycle["status"]) {
    return {
      id,
      type: "tool" as const,
      tool: {
        id,
        name,
        argumentsPreview: "",
        arguments: "",
        progress: "",
        output: "",
        status
      }
    };
  }

  it("连续已完成读取提前收成一组", () => {
    const segments = splitWaveForEarlyReadFold([
      toolWavePart("a", "read_file", "completed"),
      toolWavePart("b", "read_file", "completed"),
      toolWavePart("c", "read_file", "completed")
    ]);
    expect(segments).toEqual([
      { kind: "readFold", parts: [expect.objectContaining({ id: "a" }), expect.objectContaining({ id: "b" }), expect.objectContaining({ id: "c" })] }
    ]);
  });

  it("单独一条已完成读取且下一条读取仍在执行时也提前折叠", () => {
    const segments = splitWaveForEarlyReadFold([
      toolWavePart("a", "read_file", "completed"),
      toolWavePart("b", "read_file", "running")
    ]);
    expect(segments.map((item) => item.kind)).toEqual(["readFold", "cards"]);
    expect(segments[0]?.parts.map((part) => part.id)).toEqual(["a"]);
    expect(segments[1]?.parts.map((part) => part.id)).toEqual(["b"]);
  });

  it("单独一条已完成读取且后面不是读取时仍逐条展示", () => {
    const segments = splitWaveForEarlyReadFold([
      toolWavePart("a", "read_file", "completed"),
      toolWavePart("b", "write_file", "completed")
    ]);
    expect(segments).toHaveLength(1);
    expect(segments[0]?.kind).toBe("cards");
    expect(segments[0]?.parts.map((part) => part.id)).toEqual(["a", "b"]);
  });

  it("写入打断连续读取折叠", () => {
    const segments = splitWaveForEarlyReadFold([
      toolWavePart("a", "read_file", "completed"),
      toolWavePart("b", "read_file", "completed"),
      toolWavePart("c", "write_file", "completed"),
      toolWavePart("d", "read_file", "completed"),
      toolWavePart("e", "grep", "completed")
    ]);
    expect(segments.map((item) => item.kind)).toEqual(["readFold", "cards", "readFold"]);
    expect(segments[0]?.parts.map((part) => part.id)).toEqual(["a", "b"]);
    expect(segments[1]?.parts.map((part) => part.id)).toEqual(["c"]);
    expect(segments[2]?.parts.map((part) => part.id)).toEqual(["d", "e"]);
  });

  it("失败的读取不并入折叠组", () => {
    const segments = splitWaveForEarlyReadFold([
      toolWavePart("a", "read_file", "completed"),
      toolWavePart("b", "read_file", "failed"),
      toolWavePart("c", "read_file", "completed")
    ]);
    expect(segments).toHaveLength(1);
    expect(segments[0]?.kind).toBe("cards");
    expect(segments[0]?.parts.map((part) => part.id)).toEqual(["a", "b", "c"]);
  });
});
