import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";

describe("usage session ranking API", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("sends ranking controls alongside filters and independent log pagination", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response("{}", { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    await api.usage.stats({
      range: "30d", source: "chat", provider_search: "provider & one",
      limit: 15, offset: 30, session_sort: "billable_tokens", session_limit: 20,
    });
    const url = new URL(fetchMock.mock.calls[0][0], "http://localhost");
    expect(url.pathname).toBe("/api/usage/stats");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      range: "30d", source: "chat", provider_search: "provider & one",
      limit: "15", offset: "30", session_sort: "billable_tokens", session_limit: "20",
    });
  });
});
