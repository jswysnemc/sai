import { describe, expect, it } from "vitest";
import type { WorkspacePanelTab } from "../workspace/workspace-tab";
import { ensureTerminalTab } from "./terminal-tab-state";

describe("ensureTerminalTab", () => {
  it("keeps one tab when activation and add flow target the same terminal", () => {
    const tab: WorkspacePanelTab = {
      id: "terminal:terminal-1",
      type: "terminal",
      title: "Terminal",
      terminalId: "terminal-1",
      closable: true
    };

    const activated = ensureTerminalTab([], tab);
    const added = ensureTerminalTab(activated, tab);

    expect(added).toEqual([tab]);
  });
});
