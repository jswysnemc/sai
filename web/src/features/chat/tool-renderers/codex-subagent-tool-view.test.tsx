import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ToolLifecycleCard } from "../tool-lifecycle-card";
import type { ToolLifecycle } from "../run-event-reducer";
import { CodexSubagentToolView } from "./codex-subagent-tool-view";

const tool: ToolLifecycle = {
  id: "subagent-1",
  name: "Start subagent audit",
  argumentsPreview: "",
  arguments: JSON.stringify({
    agentThreadId: "thread-audit",
    agentPath: "/root/audit",
    activityKind: "started"
  }),
  progress: "",
  output: "Subagent accepted the task",
  status: "completed"
};

describe("CodexSubagentToolView", () => {
  it("shows semantic activity details instead of relying on raw JSON", () => {
    const html = renderToStaticMarkup(
      <CodexSubagentToolView
        tool={tool}
        activity={{
          threadId: "thread-audit",
          path: "/root/audit",
          name: "audit",
          activity: "started"
        }}
        expanded
        onToggle={() => undefined}
      />
    );

    expect(html).toContain("子智能体");
    expect(html).toContain("audit");
    expect(html).toContain("thread-audit");
    expect(html).toContain("/root/audit");
    expect(html).toContain("已启动");
    expect(html).toContain("已完成");
    expect(html).not.toContain("agentThreadId");
  });

  it("is selected by the shared tool lifecycle card", () => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const html = renderToStaticMarkup(
      <QueryClientProvider client={queryClient}>
        <ToolLifecycleCard tool={tool} />
      </QueryClientProvider>
    );

    expect(html).toContain("子智能体");
    expect(html).toContain("audit");
    expect(html).toContain("已启动");
    expect(html).not.toContain("agentThreadId");
  });
});
