import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { AgentToolPermissions } from "./agent-tool-permissions";
import { DEFERRED_ALL_NON_BASE } from "./agent-tool-mode-state";

const tools = [
  { name: "read_file", group: "base", resident: true, group_label: "基础操作", group_rank: 0, description: "读取文件内容" },
  { name: "edit_file", group: "base", resident: true, group_label: "基础操作", group_rank: 0, description: "编辑文件内容" },
  { name: "web_search", group: "web", group_label: "网页检索", group_rank: 2, description: "搜索网页内容" }
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
    expect(html).toContain("非常驻按需");
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

  it("常驻工具即使不在基础分组里也显示为启用", () => {
    // 网格工具自成一组但属于常驻集合，按分组推断会被误判成按需
    const html = renderToStaticMarkup(
      <AgentToolPermissions
        tools={[
          { name: "session_probe", group: "mesh", resident: true, group_label: "会话网格", group_rank: 12, description: "列出会话" },
          { name: "web_search", group: "web", group_label: "网页检索", group_rank: 2, description: "搜索网页内容" }
        ]}
        enabled={[]}
        deferred={[DEFERRED_ALL_NON_BASE]}
        onChange={vi.fn()}
      />
    );

    expect(html).toContain('data-group="mesh"');
    expect(html).toContain("启用 1 · 按需 1 · 关闭 0");
  });

  it("把 SSH 组排在基础组之后，并说明用户与模型的分工", () => {
    const html = renderToStaticMarkup(
      <MemoryRouter>
        <AgentToolPermissions
          tools={[
            ...tools,
            {
              name: "ssh_list_hosts",
              group: "ssh",
              group_label: "SSH 远程",
              group_label_en: "SSH",
              group_hint: "主机和密码在「设置 → SSH」里由你配置和输入。",
              group_hint_en: "You add hosts and type passwords in Settings → SSH.",
              group_settings_path: "/settings/ssh",
              group_rank: 1,
              description: "列出已配置的 SSH 主机"
            },
            {
              name: "ssh_run_command",
              group: "ssh",
              group_label: "SSH 远程",
              group_rank: 1,
              description: "在远程主机执行命令"
            }
          ]}
          enabled={[]}
          deferred={[]}
          onChange={vi.fn()}
        />
      </MemoryRouter>
    );

    const sshAt = html.indexOf('data-group="ssh"');
    const webAt = html.indexOf('data-group="web"');
    expect(sshAt).toBeGreaterThan(-1);
    expect(webAt).toBeGreaterThan(-1);
    expect(sshAt).toBeLessThan(webAt);
    expect(html).toContain("SSH 远程");
    expect(html).toContain("主机和密码在「设置 → SSH」里由你配置和输入。");
    expect(html).toContain('href="/settings/ssh"');
    expect(html).toContain("配置主机");
  });
});
