import { describe, expect, it } from "vitest";
import { initialMobileWorkbenchState, reduceMobileWorkbenchState } from "./mobile-workbench-state";

describe("mobile workbench state", () => {
  it("通过 Logo 切换并通过遮罩关闭会话侧栏", () => {
    const opened = reduceMobileWorkbenchState(initialMobileWorkbenchState, { type: "toggle-sidebar" });
    expect(opened.sidebarOpen).toBe(true);
    expect(reduceMobileWorkbenchState(opened, { type: "close-sidebar" }).sidebarOpen).toBe(false);
  });

  it("允许聊天、编辑器和终端依次成为当前面板", () => {
    const workspace = reduceMobileWorkbenchState(initialMobileWorkbenchState, { type: "show-pane", pane: "workspace" });
    const terminal = reduceMobileWorkbenchState(workspace, { type: "show-pane", pane: "terminal" });
    const chat = reduceMobileWorkbenchState(terminal, { type: "show-pane", pane: "chat" });

    expect(workspace.pane).toBe("workspace");
    expect(terminal.pane).toBe("terminal");
    expect(chat.pane).toBe("chat");
  });
});
