import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ToolDiffBadge } from "./tool-diff-badge";

describe("ToolDiffBadge", () => {
  it("renders both counts with their own colors", () => {
    const html = renderToStaticMarkup(<ToolDiffBadge added={9} removed={4} />);

    expect(html).toContain("+9");
    expect(html).toContain("-4");
    expect(html).toContain("text-diff-added");
    expect(html).toContain("text-diff-removed");
  });

  it("omits a side that has no changes", () => {
    const onlyAdded = renderToStaticMarkup(<ToolDiffBadge added={3} removed={0} />);

    expect(onlyAdded).toContain("+3");
    expect(onlyAdded).not.toContain("text-diff-removed");
  });

  it("renders nothing when there is no change at all", () => {
    // 空徽章会在摘要行留下一个无意义的间隙
    expect(renderToStaticMarkup(<ToolDiffBadge added={0} removed={0} />)).toBe("");
  });

  it("aligns digits so stacked files stay comparable", () => {
    const html = renderToStaticMarkup(<ToolDiffBadge added={120} removed={7} />);

    expect(html).toContain("tabular-nums");
  });
});
