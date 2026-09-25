import { describe, expect, it } from "vitest";
import { resolveWorkspaceImage } from "./markdown-image-url";

describe("resolveWorkspaceImage", () => {
  it("以 Markdown 文件所在目录解析相对路径", () => {
    expect(resolveWorkspaceImage("docs/guide.md", "./img/a.png")).toBe(`/api/workspace/image?path=${encodeURIComponent("docs/img/a.png")}`);
    expect(resolveWorkspaceImage("docs/guide.md", "../pics/b.png")).toBe(`/api/workspace/image?path=${encodeURIComponent("pics/b.png")}`);
  });

  it("绝对地址原样放行，越出根目录与其他协议返回 null", () => {
    expect(resolveWorkspaceImage("a.md", "https://x.y/z.png")).toBe("https://x.y/z.png");
    expect(resolveWorkspaceImage("a.md", "../out.png")).toBeNull();
    expect(resolveWorkspaceImage("a.md", "javascript:alert(1)")).toBeNull();
  });
});
