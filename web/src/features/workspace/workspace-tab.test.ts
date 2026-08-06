import { afterEach, describe, expect, it, vi } from "vitest";
import { createWorkspacePanelTab } from "./workspace-tab";

afterEach(() => vi.unstubAllGlobals());

describe("createWorkspacePanelTab", () => {
  it("保留工作区外绝对路径并生成稳定文件标签", () => {
    const tab = createWorkspacePanelTab("files", { path: "/tmp/external/report.md" });

    expect(tab).toMatchObject({
      id: "file:/tmp/external/report.md",
      type: "files",
      title: "report.md",
      path: "/tmp/external/report.md"
    });
  });

  it("同一文件的被动 Diff 使用稳定标识并保留补丁", () => {
    const first = createWorkspacePanelTab("diff", {
      path: "src/main.rs",
      diffSource: "@@ -1 +1 @@\n-old\n+new"
    });
    const second = createWorkspacePanelTab("diff", {
      path: "src/main.rs",
      diffSource: "@@ -1 +1 @@\n-before\n+after"
    });

    expect(first.id).toBe("diff:src/main.rs");
    expect(second.id).toBe(first.id);
    expect(second.diffSource).toContain("+after");
  });

  it("空编辑器标签保持独立标识", () => {
    vi.stubGlobal("crypto", { randomUUID: vi.fn().mockReturnValueOnce("one").mockReturnValueOnce("two") });

    const first = createWorkspacePanelTab("files");
    const second = createWorkspacePanelTab("files");

    expect(first.id).toBe("files:one");
    expect(second.id).toBe("files:two");
  });

  it("旁路对话标签使用请求标识且保留冻结上下文", () => {
    const request = {
      id: "question-one",
      title: "解释当前回复",
      workspaceId: "workspace-1",
      sourceSessionId: "session-1",
      sourceTurnId: "turn-1",
      context: "上下文",
      mode: "yolo" as const,
      thinkingLevel: "auto" as const
    };

    const tab = createWorkspacePanelTab("side-chat", { sideConversation: request });

    expect(tab.id).toBe("side-chat:question-one");
    expect(tab.sideConversation).toBe(request);
  });
});
