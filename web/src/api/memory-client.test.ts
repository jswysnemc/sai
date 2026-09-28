import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";

describe("memory scope API", () => {
  afterEach(() => vi.unstubAllGlobals());
  it("preserves both workspace and exact scope when reading or removing an entry", async () => {
    const fetchMock = vi.fn().mockImplementation(async () => new Response("{}", { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    await api.memory.show("shared name", { workspace: "project-a", scope: "global" });
    await api.memory.remove("shared name", { workspace: "project-a", scope: "project" });
    const urls = fetchMock.mock.calls.map(([url]) => new URL(url, "http://localhost"));
    expect(urls[0].searchParams.get("scope")).toBe("global");
    expect(urls[1].searchParams.get("scope")).toBe("project");
    expect(urls.every((url) => url.searchParams.get("workspace") === "project-a")).toBe(true);
    expect(urls[1].pathname).toBe("/api/memory/entries/shared%20name");
    expect(fetchMock.mock.calls[1][1].method).toBe("DELETE");
  });
});
