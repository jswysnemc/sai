import { describe, expect, it, vi } from "vitest";
import { sidebarBlankMenuItems } from "./sidebar-blank-menu";

const t = (en: string) => en;
const base = () => ({
  mode: "sessions" as const, historyOpen: false, hasSessions: true, createPending: false,
  onNewSession: vi.fn(), onSearch: vi.fn(), onSelectSessions: vi.fn(), onToggleHistory: vi.fn(), onAddWorkspace: vi.fn(), onRefresh: vi.fn(),
  icons: { create: null, search: null, select: null, history: null, workspace: null, refresh: null }
});

describe("sidebar blank menu", () => {
  it("offers session actions in session mode", () => {
    const actions = base();
    const items = sidebarBlankMenuItems(actions, t);
    expect(items.map((item) => item.id)).toEqual(["new-session", "search", "select", "history", "refresh"]);
    expect(items.find((item) => item.id === "history")?.label).toBe("Expand History");
    items.find((item) => item.id === "select")?.onSelect();
    expect(actions.onSelectSessions).toHaveBeenCalled();
  });

  it("offers workspace actions in workspace mode", () => {
    const items = sidebarBlankMenuItems({ ...base(), mode: "workspaces" }, t);
    expect(items.map((item) => item.id)).toEqual(["new-session", "add-workspace", "search", "refresh"]);
  });

  it("disables selection without sessions and creation while pending", () => {
    const items = sidebarBlankMenuItems({ ...base(), hasSessions: false, createPending: true }, t);
    expect(items.find((item) => item.id === "select")?.disabled).toBe(true);
    expect(items.find((item) => item.id === "new-session")?.disabled).toBe(true);
  });
});
