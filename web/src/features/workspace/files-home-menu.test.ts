import { describe, expect, it, vi } from "vitest";
import { filesHomeMenuItems } from "./files-home-menu";

const t = (en: string) => en;
const actions = () => ({
  open: vi.fn(), reveal: vi.fn(), copyPath: vi.fn(), forget: vi.fn(),
  createFile: vi.fn(), browse: vi.fn(), clearRecents: vi.fn(), hasRecents: true,
  icons: { open: null, reveal: null, copy: null, remove: null, create: null, browse: null, clear: null }
});

describe("files home context menu", () => {
  it("offers file actions on a recent entry", () => {
    const handlers = actions();
    const items = filesHomeMenuItems({ x: 0, y: 0, path: "src/a.ts", recent: true }, handlers, t);
    expect(items.map((item) => item.id)).toEqual(["open", "reveal", "copy-path", "copy-relative", "forget"]);
    items.find((item) => item.id === "forget")?.onSelect();
    expect(handlers.forget).toHaveBeenCalledWith("src/a.ts");
  });

  it("omits recent-only actions for search results and folders", () => {
    const items = filesHomeMenuItems({ x: 0, y: 0, path: "src", directory: true }, actions(), t);
    expect(items.map((item) => item.id)).toEqual(["open", "copy-path", "copy-relative"]);
  });

  it("offers page actions on blank space and disables clearing an empty list", () => {
    const items = filesHomeMenuItems({ x: 0, y: 0 }, { ...actions(), hasRecents: false }, t);
    expect(items.map((item) => item.id)).toEqual(["new-file", "browse", "clear"]);
    expect(items.find((item) => item.id === "clear")?.disabled).toBe(true);
  });
});
