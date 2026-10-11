import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SettingsField } from "./settings-field";
import { SwitchField } from "./switch";

describe("SettingsField", () => {
  it("renders a responsive row with the title left and the control right", () => {
    const html = renderToStaticMarkup(
      <SettingsField label="语言" hint="仅保存在当前浏览器">
        <input />
      </SettingsField>
    );
    expect(html).toContain("sk-row");
    expect(html).toContain("is-responsive");
    expect(html).toContain("sk-row-title");
    expect(html).toContain("语言");
    expect(html).toContain("仅保存在当前浏览器");
    expect(html).toContain("sk-row-control");
    expect(html.indexOf("sk-row-title")).toBeLessThan(html.indexOf("sk-row-control"));
  });

  it("stacks a full-width field", () => {
    const html = renderToStaticMarkup(
      <SettingsField label="文档" span="full">
        <textarea />
      </SettingsField>
    );
    expect(html).toContain("is-stack");
  });
});

describe("SwitchField", () => {
  it("keeps the switch after the title so the control sits on the right", () => {
    const html = renderToStaticMarkup(
      <SwitchField label="启用桌面操作" hint="默认关闭" checked={false} onChange={() => undefined} />
    );
    expect(html).toContain("sk-row");
    expect(html).toContain("is-inline");
    expect(html).toContain("启用桌面操作");
    expect(html.indexOf("启用桌面操作")).toBeLessThan(html.indexOf("role=\"switch\""));
  });
});
