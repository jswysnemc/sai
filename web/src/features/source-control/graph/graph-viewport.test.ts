import { describe, expect, it } from "vitest";
import { snapshotGraphViewport } from "./graph-viewport";

describe("snapshotGraphViewport", () => {
  it("keeps scroll metrics after the event target is released", () => {
    const target = { scrollTop: 168, clientHeight: 560 };
    const viewport = snapshotGraphViewport(target);

    target.scrollTop = 0;
    target.clientHeight = 0;

    expect(viewport).toEqual({ scrollTop: 168, height: 560 });
  });
});
