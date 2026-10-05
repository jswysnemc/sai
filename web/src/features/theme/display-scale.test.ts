import { describe, expect, it } from "vitest";
import { FONT_SCALE_OPTIONS, loadDisplayScale, PAGE_ZOOM_OPTIONS } from "./display-scale";

describe("display scale", () => {
  it("falls back to 100% when storage is empty", () => {
    expect(loadDisplayScale()).toEqual({ fontScale: 100, pageZoom: 100 });
  });

  it("exposes compact font and zoom steps", () => {
    expect(FONT_SCALE_OPTIONS.map((item) => item.value)).toEqual(["87.5", "100", "112.5", "125"]);
    expect(PAGE_ZOOM_OPTIONS.map((item) => item.value)).toEqual(["90", "100", "110", "125"]);
  });
});
