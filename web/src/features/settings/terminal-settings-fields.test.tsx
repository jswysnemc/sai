import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { AppConfig } from "../../api/contracts";
import { TerminalSettingsFields } from "./terminal-settings-fields";

describe("TerminalSettingsFields", () => {
  it("展示已配置的网页终端 Shell 和平台默认值说明", () => {
    const config = {
      active_provider: "test",
      providers: [],
      gateways: { qq: {}, weixin: {} },
      terminal: { shell: "powershell.exe" }
    } as unknown as AppConfig;

    const html = renderToStaticMarkup(
      <TerminalSettingsFields config={config} onConfigChange={vi.fn()} />
    );

    expect(html).toContain("终端 Shell");
    expect(html).toContain('value="powershell.exe"');
    expect(html).toContain("Windows 留空使用 PowerShell");
  });
});
