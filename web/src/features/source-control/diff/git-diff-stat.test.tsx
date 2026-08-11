import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { GitDiffStat } from "./git-diff-stat";

vi.mock("../../i18n/use-i18n", () => ({
  useI18n: () => ({
    t: (_en: string, zh: string) => zh,
  }),
}));

describe("GitDiffStat", () => {
  it("keeps short stats fully visible without a toggle", () => {
    const stat = ["a.rs | 1 +", "1 file changed, 1 insertion(+)"].join("\n");
    const html = renderToStaticMarkup(<GitDiffStat stat={stat} />);
    expect(html).toContain("a.rs | 1 +");
    expect(html).not.toContain("git-diff-stat-toggle");
  });

  it("collapses long stats to the summary line by default", () => {
    const files = Array.from({ length: 10 }, (_, index) => `src/f${index}.rs | ${index} +`);
    const summary = "10 files changed, 45 insertions(+), 12 deletions(-)";
    const html = renderToStaticMarkup(<GitDiffStat stat={[...files, summary].join("\n")} />);
    expect(html).toContain("git-diff-stat-toggle");
    expect(html).toContain(summary);
    expect(html).toContain("展开 10 个文件");
    expect(html).not.toContain("src/f0.rs | 0 +");
  });
});
