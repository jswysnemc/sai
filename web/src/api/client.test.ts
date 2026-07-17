import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import type { RunModelSelection } from "./contracts";

describe("system usage api", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("requests usage for the selected chat model", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({}), {
      status: 200,
      headers: { "Content-Type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);
    const selection: RunModelSelection = { providerId: "provider-a", model: "model/large" };

    await api.system.usage(selection);

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/system/usage?provider_id=provider-a&model=model%2Flarge",
      expect.objectContaining({ credentials: "same-origin" })
    );
  });
});

describe("session retry api", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("requests a context-only rollback for the selected turn", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ removed: 1, prompt: "retry" }), {
      status: 200,
      headers: { "Content-Type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await api.sessions.rollback("session-1", "run-1");

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/sessions/session-1/rollback",
      expect.objectContaining({ method: "POST", body: JSON.stringify({ turn_id: "run-1" }) })
    );
  });
});
