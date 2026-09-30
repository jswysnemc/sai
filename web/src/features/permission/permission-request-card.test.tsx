import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PermissionRequestCard } from "./permission-request-card";

describe("PermissionRequestCard", () => {
  it("renders a command approval inside the message flow without raw JSON", () => {
    const html = renderToStaticMarkup(
      <PermissionRequestCard request={{
        id: "permission",
        session_id: "session",
        tool: "run_command",
        arguments: "{\"command\":\"cargo test\",\"cwd\":\"/workspace\"}"
      }} active />
    );

    expect(html).toContain("需要权限");
    expect(html).toContain("cargo test");
    expect(html).toContain("允许一次");
    expect(html).toContain("拒绝并回复");
    expect(html).not.toContain("{&quot;command&quot;");
    expect(html).not.toContain("role=\"dialog\"");
  });

  it("renders a replayed decision as resolved and non-interactive", () => {
    const html = renderToStaticMarkup(
      <PermissionRequestCard
        request={{ id: "permission", session_id: "session", tool: "edit_file", arguments: "{\"path\":\"src/main.rs\"}" }}
        decision={{ decision: "deny", reply: "保留该文件" }}
        active={false}
      />
    );

    expect(html).toContain("已拒绝");
    expect(html).toContain("保留该文件");
    expect(html).not.toContain("允许一次");
  });

  it("shows whether an approved command leaves the sandbox", () => {
    const html = renderToStaticMarkup(
      <PermissionRequestCard request={{
        id: "permission",
        session_id: "session",
        tool: "run_command",
        arguments: "{\"command\":\"curl https://x.dev\"}",
        sandbox: { kind: "escalated", backend: "bwrap", network: false, reasons: ["network"], justification: "拉取接口定义" }
      }} active />
    );

    expect(html).toContain("批准后在沙箱外执行");
    expect(html).toContain("原因：联网");
    expect(html).toContain("理由：拉取接口定义");
    expect(html).toContain('data-sandbox-kind="escalated"');
  });
});
