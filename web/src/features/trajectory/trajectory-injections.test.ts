import { describe, expect, it } from "vitest";
import { trajectoryInjections } from "./trajectory-injections";

describe("独立注入轨迹", () => {
  it("同时保留 Jev 和普通上下文，并将 Jev 放在前面", () => {
    const jev = `<jev-exposed-capabilities>${JSON.stringify({ ok: true, tools: [{ name: "read_file" }], skills: [] })}</jev-exposed-capabilities>`;
    const context = "<context-state>workspace changed</context-state>";
    const entries = trajectoryInjections(`${context}\n${jev}`);
    expect(entries.map((entry) => entry.id)).toEqual(["jev", "injected"]);
    expect(entries[0].content).toBe(jev);
    expect(entries[0].exposure).toBeDefined();
    expect(entries[1].content).toBe(context);
  });

  it("无法解析的 Jev 内容仍保留在原始注入详情", () => {
    const content = "<jev-exposed-capabilities>invalid payload</jev-exposed-capabilities>";
    expect(trajectoryInjections(content).map((entry) => entry.content).join("\n")).toContain(content);
  });
});
