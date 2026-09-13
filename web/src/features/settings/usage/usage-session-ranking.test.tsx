import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { UsageSessionStats } from "../../../api/contracts/usage";
import { UsageSessionRanking } from "./usage-session-ranking";

/** 构造包含原始量和折算量的会话记录；参数为覆盖字段，返回测试记录。 */
function session(overrides: Partial<UsageSessionStats> = {}): UsageSessionStats {
  return {
    session_id: "session-1", workspace_id: "workspace-1", title: "分析性能问题", session_available: true,
    total_requests: 5, successful_requests: 4, failed_requests: 1, missing_usage_requests: 1,
    provider_reported_requests: 4, total_tokens: 120_000, input_tokens: 100_000, output_tokens: 20_000,
    cache_read_tokens: 80_000, cache_write_tokens: 0, billable_input_tokens: 28_000,
    billable_total_tokens: 48_000, ...overrides,
  };
}

/** 渲染中文排行；参数为会话数组，返回静态标记。 */
function render(rows: UsageSessionStats[]) {
  return renderToStaticMarkup(<UsageSessionRanking
    rows={rows} total={rows.length} sort="total_tokens" limit={10}
    onSortChange={() => undefined} onLimitChange={() => undefined}
    t={(_en, zh) => zh} locale="zh-CN"
  />);
}

describe("session consumption ranking", () => {
  it("shows identifiable sessions and separates reported from billable totals", () => {
    const html = render([session()]);
    expect(html).toContain("分析性能问题");
    expect(html).toContain("session-1");
    expect(html).toContain("workspace-1");
    expect(html).toContain("120.0K");
    expect(html).toContain("48.0K");
    expect(html).toContain("1 次未上报用量");
    expect(html).toContain("会话排名指标");
    expect(html).toContain("排行数量");
    expect(html).not.toContain("<select");
  });

  it("keeps records whose session was deleted and escapes session titles", () => {
    const html = render([session({ title: null, session_available: false }), session({ session_id: "second", title: "<script>title</script>" })]);
    expect(html).toContain("session-1");
    expect(html).toContain("&lt;script&gt;title&lt;/script&gt;");
    expect(html).not.toContain("<script>");
  });

  it("explains the empty filtered result", () => {
    expect(render([])).toContain("当前筛选范围内暂无会话用量");
    expect(render([])).not.toContain("<table");
  });
});
