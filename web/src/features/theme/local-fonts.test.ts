import { describe, expect, it } from "vitest";
import { filterFontFamilies, uniqueFontFamilies } from "./local-fonts";

describe("local font families", () => {
  it("keeps one entry per family and sorts by locale", () => {
    expect(uniqueFontFamilies([
      { family: "Fira Code" },
      { family: " Fira Code " },
      { family: "Sarasa Mono SC" },
      { family: "" },
      { family: "Inter" }
    ])).toEqual(["Fira Code", "Inter", "Sarasa Mono SC"]);
  });

  it("filters by name and reports families hidden past the visible limit", () => {
    const families = Array.from({ length: 205 }, (_, index) => `Font ${String(index).padStart(3, "0")}`);
    const all = filterFontFamilies(families, "");
    const matched = filterFontFamilies(["Sarasa Mono SC", "Noto Sans SC", "Inter"], "sans");

    expect(all.visible).toHaveLength(200);
    expect(all.hidden).toBe(5);
    expect(matched.visible).toEqual(["Noto Sans SC"]);
    expect(matched.hidden).toBe(0);
  });
});
