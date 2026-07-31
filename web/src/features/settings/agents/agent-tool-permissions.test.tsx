import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { AgentToolPermissions } from "./agent-tool-permissions";

const tools = [
  { name: "read_file", group: "base", group_label: "基础操作", description: "读取文件内容" },
  { name: "edit_file", group: "base", group_label: "基础操作", description: "编辑文件内容" },
  { name: "web_search", group: "web", group_label: "网页检索", description: "搜索网页内容" }
];

describe("AgentToolPermissions", () => {
  it("渲染搜索、状态筛选与批量操作", () => {
    const html = renderToStaticMarkup(
      <AgentToolPermissions
        tools={tools}
        enabled={["read_file", "web_search"]}
        deferred={["web_search"]}
        onChange={vi.fn()}
      />
    );

    expect(html).toContain('placeholder="搜索工具、分组或说明"');
    expect(html).toContain("全部启用");
    expect(html).toContain("非基础按需");
    expect(html).toContain('aria-label="筛选工具状态"');
  });

  it("按三段状态渲染每个工具的当前档位", () => {
    const html = renderToStaticMarkup(
      <AgentToolPermissions
        tools={tools}
        enabled={["read_file", "web_search"]}
        deferred={["web_search"]}
        onChange={vi.fn()}
      />
    );

    // read_file 启用、web_search 按需、edit_file 未在白名单内
    expect(html).toContain('data-mode="on"');
    expect(html).toContain('data-mode="load"');
    expect(html).toContain('data-mode="off"');
    expect(html).toContain("启用 1 · 按需 1 · 关闭 1");
  });

  it("为分组与单项都提供可访问名称", () => {
    const html = renderToStaticMarkup(
      <AgentToolPermissions tools={tools} enabled={[]} deferred={[]} onChange={vi.fn()} />
    );

    expect(html).toContain('aria-label="设置网页检索分组的权限"');
    expect(html).toContain('aria-label="设置 web_search 的权限"');
  });
});
