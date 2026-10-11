import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SettingsPageHeader } from "./settings-page-header";

describe("SettingsPageHeader", () => {
  it("renders the pane title and description", () => {
    const html = renderToStaticMarkup(
      <SettingsPageHeader title="运行时" description="会话、权限、终端与显示" action={<span>即时生效</span>} />
    );
    expect(html).toContain("settings-page-header");
    expect(html).toContain("<h1");
    expect(html).toContain("运行时");
    expect(html).toContain("会话、权限、终端与显示");
    expect(html).toContain("即时生效");
  });

  it("omits the description when none is given", () => {
    const html = renderToStaticMarkup(<SettingsPageHeader title="运行时" />);
    expect(html).not.toContain("settings-page-header-desc");
  });
});
