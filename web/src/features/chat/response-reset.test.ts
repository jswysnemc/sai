import { describe, expect, it } from "vitest";
import { initialRunState } from "./run-event-reducer";
import { resetResponse } from "./response-reset";

describe("response retry rollback", () => {
  it("keeps previous completed output and removes Unicode fragments from the failed request", () => {
    const result = resetResponse({
      ...initialRunState,
      content: "之前正文失效片段",
      reasoning: "之前思考新思考",
      parts: [
        { id: "earlier", type: "text", source: "之前正文" },
        { id: "reasoning", type: "reasoning", source: "之前思考新思考", startedAt: "now" },
        { id: "failed", type: "text", source: "失效片段" }
      ]
    }, 4, 3, false);
    expect(result.content).toBe("之前正文");
    expect(result.reasoning).toBe("之前思考");
    expect(result.parts).toHaveLength(2);
  });
});
