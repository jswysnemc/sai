import { describe, expect, it } from "vitest";
import {
  diffToolWaveEvents,
  enqueueToolWaveEvents,
  isToolWaveActive
} from "./tool-wave-queue";

describe("diffToolWaveEvents", () => {
  it("emits start when a tool first appears as active", () => {
    expect(diffToolWaveEvents({}, [
      { id: "a", status: "preparing" },
      { id: "b", status: "running" }
    ])).toEqual([
      { id: "a", kind: "start" },
      { id: "b", kind: "start" }
    ]);
  });

  it("does not emit start for preparing → running", () => {
    expect(diffToolWaveEvents(
      { a: "preparing" },
      [{ id: "a", status: "running" }]
    )).toEqual([]);
  });

  it("emits end when an active tool completes or fails", () => {
    expect(diffToolWaveEvents(
      { a: "running", b: "preparing" },
      [
        { id: "a", status: "completed" },
        { id: "b", status: "failed" }
      ]
    )).toEqual([
      { id: "a", kind: "end" },
      { id: "b", kind: "end" }
    ]);
  });

  it("keeps concurrent start and end in list order", () => {
    expect(diffToolWaveEvents(
      { a: "running" },
      [
        { id: "a", status: "completed" },
        { id: "b", status: "running" }
      ]
    )).toEqual([
      { id: "a", kind: "end" },
      { id: "b", kind: "start" }
    ]);
  });

  it("ignores tools that first appear already finished", () => {
    expect(diffToolWaveEvents({}, [
      { id: "a", status: "completed" }
    ])).toEqual([]);
  });
});

describe("enqueueToolWaveEvents", () => {
  it("appends start and end even for the same tool", () => {
    expect(enqueueToolWaveEvents(
      [{ id: "a", kind: "start" }],
      [{ id: "a", kind: "end" }]
    )).toEqual([
      { id: "a", kind: "start" },
      { id: "a", kind: "end" }
    ]);
  });

  it("drops consecutive duplicate events from streaming patches", () => {
    expect(enqueueToolWaveEvents(
      [{ id: "a", kind: "start" }],
      [{ id: "a", kind: "start" }, { id: "b", kind: "start" }]
    )).toEqual([
      { id: "a", kind: "start" },
      { id: "b", kind: "start" }
    ]);
  });
});

describe("isToolWaveActive", () => {
  it("treats preparing and running as active", () => {
    expect(isToolWaveActive("preparing")).toBe(true);
    expect(isToolWaveActive("running")).toBe(true);
    expect(isToolWaveActive("completed")).toBe(false);
    expect(isToolWaveActive("failed")).toBe(false);
  });
});
