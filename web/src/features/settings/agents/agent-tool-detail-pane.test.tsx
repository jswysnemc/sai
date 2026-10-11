import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentToolDetailPane } from "./agent-tool-detail-pane";
import type { ToolModeLabels } from "./agent-tool-mode-group";

const mockLabels: ToolModeLabels = [
  { value: "on", label: "启用", title: "启用" },
  { value: "load", label: "按需", title: "按需" },
  { value: "off", label: "关闭", title: "关闭" }
];

describe("AgentToolDetailPane", () => {
  it("无选中工具时展示空态引导", () => {
    const html = renderToStaticMarkup(
      <AgentToolDetailPane
        tool={null}
        modeLabels={mockLabels}
        onModeChange={() => undefined}
      />
    );
    expect(html).toContain("点击左侧工具可在此查看其详细参数定义");
  });

  it("选中工具时展示工具名称、参数列表与属性", () => {
    const html = renderToStaticMarkup(
      <AgentToolDetailPane
        tool={{
          name: "run_command",
          group: "base",
          group_label: "基础操作",
          resident: true,
          description: "使用已配置的 POSIX Shell 执行命令",
          full_description: "完整描述：在系统中执行命令",
          parameters: {
            type: "object",
            properties: {
              command: {
                type: "string",
                description: "完整命令或脚本"
              },
              timeout_seconds: {
                type: "integer",
                description: "超时时间"
              }
            },
            required: ["command"]
          }
        }}
        mode="on"
        modeLabels={mockLabels}
        onModeChange={() => undefined}
      />
    );

    expect(html).toContain("run_command");
    expect(html).toContain("基础操作");
    expect(html).toContain("常驻工具");
    expect(html).toContain("command");
    expect(html).toContain("timeout_seconds");
    expect(html).toContain("必填");
    expect(html).toContain("可选");
  });
});
