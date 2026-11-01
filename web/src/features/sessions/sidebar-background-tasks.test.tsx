import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { SessionSidebarActions } from "./session-sidebar-actions";
import { SidebarCollapsedRail } from "./sidebar-collapsed-rail";

describe("侧栏后台任务入口", () => {
  it("展开侧栏保留独立的后台任务管理按钮", () => {
    const html = renderToStaticMarkup(<SessionSidebarActions
      onNewSession={vi.fn()} onSearch={vi.fn()} onScheduledTasks={vi.fn()}
      onSkills={vi.fn()} onBackgroundTasks={vi.fn()}
    />);
    expect(html).toContain('aria-label="后台任务"');
    expect(html).toContain('aria-label="定时任务"');
  });

  it("折叠侧栏仍提供后台任务管理按钮", () => {
    const html = renderToStaticMarkup(<MemoryRouter><SidebarCollapsedRail
      onExpand={vi.fn()} onNewSession={vi.fn()} onSearch={vi.fn()} onBackgroundTasks={vi.fn()}
      newSessionPending={false} onOpenDirectory={vi.fn()}
    /></MemoryRouter>);
    expect(html).toMatch(/<button[^>]*class="ui-button[^\"]*sidebar-rail-button"[^>]*aria-label="后台任务"/);
  });
});
