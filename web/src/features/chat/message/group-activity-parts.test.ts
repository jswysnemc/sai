import { describe, expect, it } from "vitest";
import type { LiveMessagePart } from "../run-event-reducer";
import { collectWaveSecrets, collectWaveTools, countWorkItems, groupActivityParts } from "./group-activity-parts";
import type { ToolLifecycle } from "../run-event-reducer";

function tool(id: string): LiveMessagePart {
  const lifecycle: ToolLifecycle = {
    id,
    name: "read_file",
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
    expect(countWorkItems(segments[0].items)).toEqual({ reasoning: 1, tools: 2 });
    expect(collectWaveTools(segments[0].items).map((part) => part.id)).toEqual(["t1", "t2"]);
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
    expect(preamble.type === "preamble" && countWorkItems(preamble.items)).toEqual({ reasoning: 3, tools: 2 });
    expect(preamble.type === "preamble" && preamble.followedByText).toBe(true);
  });

  it("末尾的空白正文不算作后接正文", () => {
    const segments = groupActivityParts([reasoning("r1"), tool("t1"), blank("b1")]);
    expect(segments).toHaveLength(1);
    expect(segments[0].type === "preamble" && segments[0].followedByText).toBe(false);
  });
});
