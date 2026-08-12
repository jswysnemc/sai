import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { Subagent } from "../../api/contracts";
import { SubagentDetailView } from "./subagent-detail-view";

vi.mock("./use-subagent-stream", () => ({
  useSubagentStream: (subagent: Subagent) => ({
    snapshot: subagent,
    timeline: [],
    timestamp: ""
  })
}));

vi.mock("../chat/composer/composer-textarea", () => ({
  ComposerTextarea: ({
    disabled,
    placeholder,
    respondToGlobalFocus
  }: {
    disabled: boolean;
    placeholder: string;
    respondToGlobalFocus?: boolean;
  }) => (
    <div
      className="composer-editor"
      aria-disabled={disabled}
      data-respond-to-global-focus={String(respondToGlobalFocus)}
    >
      {placeholder}
    </div>
  )
}));

vi.mock("../chat/message/message-parts", () => ({ MessageParts: () => null }));
vi.mock("./subagent-stats", () => ({ SubagentStats: () => null }));

/** 构造详情输入区测试所需的最小子智能体快照。 */
function subagent(status: string): Subagent {
  return {
    id: "subagent-1",
    description: "检查工作区",
    subagent_type: "generalPurpose",
    status,
    max_steps: 20,
    started_at: 1,
    updated_at: 2,
    step: 1
  };
}

describe("SubagentDetailView composer", () => {
  it("reuses the compact main composer surface for live subagents", () => {
    const html = renderToStaticMarkup(
      <SubagentDetailView subagent={subagent("running")} onBack={() => undefined} onCancel={() => undefined} />
    );

    expect(html).toContain("composer-surface-compact composer subagent-detail-composer");
    expect(html).toContain("composer-send");
    expect(html).toContain("给子智能体留言");
    expect(html).toContain('data-respond-to-global-focus="false"');
    expect(html).not.toContain("<input");
  });

  it("keeps the shared composer visible but disabled after completion", () => {
    const html = renderToStaticMarkup(
      <SubagentDetailView subagent={subagent("completed")} onBack={() => undefined} onCancel={() => undefined} />
    );

    expect(html).toContain("composer-surface-compact composer subagent-detail-composer");
    expect(html).toContain("子智能体已结束，不再接收留言");
    expect(html).toContain('aria-disabled="true"');
  });
});
