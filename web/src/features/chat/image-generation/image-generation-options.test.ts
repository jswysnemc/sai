import { describe, expect, it } from "vitest";
import { imageResolutionsForAspectRatio } from "./image-generation-options";

describe("image generation dimensions", () => {
  it.each([
    ["1:1", ["1024x1024", "2048x2048"]],
    ["4:3", ["1536x1152"]],
    ["3:4", ["1152x1536"]],
    ["16:9", ["1536x864"]],
    ["9:16", ["864x1536"]]
  ] as const)("keeps %s aspect ratio dimensions aligned", (ratio, expected) => {
    expect(imageResolutionsForAspectRatio(ratio).map((item) => item.value)).toEqual(expected);
  });
});
