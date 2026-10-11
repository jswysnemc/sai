import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SettingsPanel } from "./settings-panel";

describe("SettingsPanel", () => {
  it("renders a 13px section label and wraps children in a group card", () => {
    const html = renderToStaticMarkup(
      <SettingsPanel title="变更视图" description="源代码管理面板如何列出变更文件。">
        <div>内容</div>
      </SettingsPanel>
    );
    expect(html).toContain("sk-panel");
    expect(html).toContain("<h3");
    expect(html).toContain("变更视图");
    expect(html).toContain("源代码管理面板如何列出变更文件。");
    expect(html).toContain("sk-group");
    expect(html).toContain("内容");
  });
});
