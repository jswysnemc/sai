import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { SettingsNav } from "./settings-nav";

/**
 * 渲染设置导航为静态标记。
 *
 * @returns HTML
 */
function renderNav(): string {
  return renderToStaticMarkup(
    <MemoryRouter>
      <SettingsNav activeSection="runtime" dirtySections={new Set()} />
    </MemoryRouter>
  );
}

describe("SettingsNav", () => {
  it("keeps search, back link, grouped sections and dirty markers reachable", () => {
    const html = renderNav();
    expect(html).toContain("settings-navigation");
    expect(html).toContain("搜索设置项");
    expect(html).toContain("返回主界面");
    expect(html).toContain("基础");
    expect(html).toContain("运行时");
    expect(html).toContain("智能体能力");
    expect(html).toContain("数据与统计");
  });

  it("marks unsaved sections", () => {
    const html = renderToStaticMarkup(
      <MemoryRouter>
        <SettingsNav activeSection="runtime" dirtySections={new Set(["runtime"])} />
      </MemoryRouter>
    );
    expect(html).toContain("settings-nav-dirty");
    expect(html).toContain("有未保存的修改");
  });
});
