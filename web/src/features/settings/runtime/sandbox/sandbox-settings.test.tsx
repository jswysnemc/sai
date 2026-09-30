import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AppConfig, SandboxStatusResponse } from "../../../../api/contracts";
import { SandboxSettings } from "./sandbox-settings";
import { SandboxStatusSummary, sandboxVerdict } from "./sandbox-status-summary";

const active: SandboxStatusResponse = {
  enabled: true,
  platform_supported: true,
  backend: "bwrap",
  available: true,
  network: "deny",
  scrub_env: true,
  writable_roots: ["/work/app", "/tmp/sai-sandbox-1000/42"],
  deny_read: ["/home/me/.ssh"],
  write_protected: ["/work/app/.git/hooks"],
  scrubbed_env: ["OPENAI_API_KEY"]
};

/** 固定返回中文的双语文本函数。 */
const zh = (_en: string, text: string) => text;

describe("sandboxVerdict", () => {
  it("distinguishes active, disabled, unsupported and broken backends", () => {
    expect(sandboxVerdict(active, zh)).toEqual({ label: "已生效", tone: "success" });
    expect(sandboxVerdict({ ...active, enabled: false }, zh).tone).toBe("warning");
    expect(sandboxVerdict({ ...active, platform_supported: false, backend: "none" }, zh).label).toBe("当前平台不支持");
    const broken = sandboxVerdict({ ...active, available: false, reason: "setting up uid map: Permission denied" }, zh);
    expect(broken.tone).toBe("danger");
    expect(broken.notice).toContain("uid map");
  });
});

describe("SandboxStatusSummary", () => {
  it("lists resolved paths and scrubbed variables", () => {
    const html = renderToStaticMarkup(<SandboxStatusSummary status={active} loading={false} failed={false} onRetry={() => undefined} />);
    expect(html).toContain("已生效");
    expect(html).toContain("bwrap");
    expect(html).toContain("断开网络");
    expect(html).toContain("/work/app/.git/hooks");
    expect(html).toContain("/home/me/.ssh");
    expect(html).toContain("OPENAI_API_KEY");
  });

  it("offers a retry when detection fails", () => {
    const html = renderToStaticMarkup(<SandboxStatusSummary loading={false} failed onRetry={() => undefined} />);
    expect(html).toContain("探测失败");
    expect(html).toContain("重试");
  });

  it("marks env scrubbing as off instead of listing variables", () => {
    const html = renderToStaticMarkup(<SandboxStatusSummary status={{ ...active, scrub_env: false }} loading={false} failed={false} onRetry={() => undefined} />);
    expect(html).not.toContain("OPENAI_API_KEY");
    expect(html).toContain("关闭");
  });
});

describe("SandboxSettings", () => {
  it("renders stored values with safe defaults", () => {
    const config = {
      active_provider: "", providers: [], gateways: {},
      sandbox: { network: "allow", writable_roots: ["~/.cargo"] }
    } as unknown as AppConfig;
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const html = renderToStaticMarkup(
      <QueryClientProvider client={client}>
        <SandboxSettings config={config} onConfigChange={() => undefined} />
      </QueryClientProvider>
    );
    expect(html).toContain("命令沙箱");
    expect(html).toContain("~/.cargo</textarea>");
    expect(html).toContain('data-config-key="sandbox.enabled"');
    expect(html).toContain('aria-checked="true"');
  });
});
