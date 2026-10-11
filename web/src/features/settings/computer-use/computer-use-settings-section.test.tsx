import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { DialogProvider } from "../../../shared/ui/dialog/dialog-provider";
import type { AppConfig } from "../../../api/contracts";
import { ComputerUseSettingsSection } from "./computer-use-settings-section";

/**
 * 渲染桌面操作设置为静态标记。
 *
 * @param config 应用配置
 * @returns HTML
 */
function render(config: AppConfig): string {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <DialogProvider>
        <ComputerUseSettingsSection config={config} onConfigChange={() => undefined} />
      </DialogProvider>
    </QueryClientProvider>
  );
}

describe("ComputerUseSettingsSection", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(JSON.stringify({
      platform: "linux",
      arch: "x86_64",
      configured_path: "cua-driver",
      installed: false,
      version_ok: false,
      min_version: "0.34.1",
      pinned_version: "0.34.1",
      install_supported: true,
      install_path: "/tmp/sai/computer-use"
    }))));
  });

  afterEach(() => vi.unstubAllGlobals());

  it("renders enable, path, timeout, and install controls", () => {
    const html = render({} as unknown as AppConfig);
    expect(html).toContain("启用桌面操作");
    expect(html).toContain("驱动路径");
    expect(html).toContain("调用超时");
    expect(html).toContain("安装 cua-driver");
    expect(html).toContain("computer_use.enabled");
    expect(html).not.toContain("alert(");
  });
});
