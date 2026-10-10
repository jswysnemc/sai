import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DisclosureItem } from "./disclosure-item";

describe("DisclosureItem", () => {
  it("把箭头放在名称和说明之后，与工具卡头部同一侧", () => {
    const html = renderToStaticMarkup(
      <ul className="disclosure-list">
        <DisclosureItem title="查看上下文块" meta="已暴露" defaultOpen>
          <p>正文</p>
        </DisclosureItem>
      </ul>
    );
    const titleAt = html.indexOf("disclosure-item-title");
    const metaAt = html.indexOf("disclosure-item-meta");
    const chevronAt = html.indexOf("disclosure-item-chevron");
    expect(titleAt).toBeGreaterThan(-1);
    expect(metaAt).toBeGreaterThan(titleAt);
    expect(chevronAt).toBeGreaterThan(metaAt);
    expect(html).toContain("disclosure-item-chevron-slot");
  });
});
