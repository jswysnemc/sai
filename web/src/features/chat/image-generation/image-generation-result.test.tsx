import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ImageGenerationResult } from "./image-generation-result";

describe("ImageGenerationResult", () => {
  it("renders the cached image returned by direct generation", () => {
    const html = renderToStaticMarkup(
      <ImageGenerationResult
        result={{
          status: "success",
          prompt: "a red square",
          output: JSON.stringify({
            type: "image_generation",
            images: [{ url: "/api/generated-images/generated-1.png", file_name: "generated-1.png" }]
          })
        }}
      />
    );

    expect(html).toContain('src="/api/generated-images/generated-1.png"');
    expect(html).toContain("a red square");
    expect(html).not.toContain("图片结果暂不可用");
  });

  it("maps a Windows local cache path to the public image route", () => {
    const html = renderToStaticMarkup(
      <ImageGenerationResult
        result={{
          status: "success",
          prompt: "a blue square",
          output: JSON.stringify({
            type: "image_generation",
            images: [{ local_path: "C:\\Users\\sai\\cache\\generated-2.png" }]
          })
        }}
      />
    );

    expect(html).toContain('src="/api/generated-images/generated-2.png"');
    expect(html).not.toContain("图片结果暂不可用");
  });

  it("maps a top-level local path when an older result shape is returned", () => {
    const html = renderToStaticMarkup(
      <ImageGenerationResult
        result={{
          status: "success",
          prompt: "a green square",
          output: JSON.stringify({ local_path: "/tmp/generated-3.webp" })
        }}
      />
    );

    expect(html).toContain('src="/api/generated-images/generated-3.webp"');
  });
});
