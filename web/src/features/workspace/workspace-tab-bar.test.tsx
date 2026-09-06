import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { WorkspaceTabBar } from "./workspace-tab-bar";

describe("WorkspaceTabBar", () => {
  it("保留可关闭标签的键盘可访问按钮", () => {
    const html = renderToStaticMarkup(
      <WorkspaceTabBar
        tabs={[
          { id: "file:README.md", type: "files", title: "README.md", path: "README.md", closable: true },
          { id: "diff", type: "diff", title: "Git", closable: false }
        ]}
        activeTabId="file:README.md"
        maximized={false}
        onActivate={vi.fn()}
        onClose={vi.fn()}
        onAdd={vi.fn()}
        onToggleMaximized={vi.fn()}
        onCollapse={vi.fn()}
      />
    );

    expect(html).toMatch(/<button[^>]*class="[^"]*workspace-tab-close[^"]*"[^>]*tabindex="0"/);
    expect(html).toContain('aria-label="关闭 README.md"');
    expect(html.match(/workspace-tab-close/g)).toHaveLength(1);
    expect(html.match(/role="tab"/g)).toHaveLength(2);
    expect(html).toContain('aria-selected="true" tabindex="0"');
    expect(html).toContain('aria-selected="false" tabindex="-1"');
  });

  it("把添加按钮放在标签滚动区外、末标签右侧", () => {
    const html = renderToStaticMarkup(
      <WorkspaceTabBar
        tabs={[
          { id: "file:README.md", type: "files", title: "README.md", path: "README.md", closable: true },
          { id: "diff", type: "diff", title: "Git", closable: false }
        ]}
        activeTabId="file:README.md"
        maximized={false}
        onActivate={vi.fn()}
        onClose={vi.fn()}
        onAdd={vi.fn()}
        onToggleMaximized={vi.fn()}
        onCollapse={vi.fn()}
      />
    );

    const rowStart = html.indexOf('class="workspace-tab-scroll-row"');
    const scrollStart = html.indexOf('class="workspace-tab-scroll"');
    const scrollEnd = html.indexOf("</div>", scrollStart);
    const actionsStart = html.indexOf('workspace-tab-actions');
    const layoutStart = html.indexOf('workspace-tab-layout');
    expect(rowStart).toBeGreaterThan(-1);
    expect(scrollStart).toBeGreaterThan(rowStart);
    expect(actionsStart).toBeGreaterThan(scrollEnd);
    expect(layoutStart).toBeGreaterThan(actionsStart);
    expect(html).toContain('aria-label="添加面板"');
  });
});
