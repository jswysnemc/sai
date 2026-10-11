import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DisclosureItem } from "./disclosure-item";

describe("DisclosureItem", () => {
  it("把展开箭头放在最左侧，其后依次是名称和说明", () => {
    const html = renderToStaticMarkup(
      <ul className="disclosure-list">
        <DisclosureItem title="查看上下文块" meta="已暴露" defaultOpen>
          <p>正文</p>
        </DisclosureItem>
      </ul>
    );
    const chevronAt = html.indexOf("disclosure-item-chevron-slot");
    const titleAt = html.indexOf("disclosure-item-title");
    const metaAt = html.indexOf("disclosure-item-meta");

    expect(chevronAt).toBeGreaterThan(-1);
    expect(titleAt).toBeGreaterThan(chevronAt);
    expect(metaAt).toBeGreaterThan(titleAt);
  });
});
