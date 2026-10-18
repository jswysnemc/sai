import { describe, expect, it } from "vitest";
import { fixedClientPoint, FONT_SCALE_OPTIONS, loadDisplayScale, PAGE_ZOOM_OPTIONS, pageZoomFactor } from "./display-scale";

describe("display scale", () => {
  it("falls back to 100% when storage is empty", () => {
    expect(loadDisplayScale()).toEqual({ fontScale: 100, pageZoom: 100 });
  });

  it("exposes compact font and zoom steps", () => {
    expect(FONT_SCALE_OPTIONS.map((item) => item.value)).toEqual(["87.5", "100", "112.5", "125"]);
    expect(PAGE_ZOOM_OPTIONS.map((item) => item.value)).toEqual(["90", "100", "110", "125"]);
  });

  it("removes the extra zoom from pointer coordinates used by fixed menus", () => {
    expect(pageZoomFactor("1.25")).toBe(1.25);
    expect(pageZoomFactor("125%")).toBe(1.25);
    expect(pageZoomFactor("normal")).toBe(1);
    expect(fixedClientPoint(500, 320, 1.25)).toEqual({ x: 400, y: 256 });
    expect(fixedClientPoint(500, 320, 1)).toEqual({ x: 500, y: 320 });
  });
});
