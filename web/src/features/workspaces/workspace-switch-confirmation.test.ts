import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api/client";
import type { Translate } from "../i18n/i18n-context";
import { getUnsavedEditorPaths, registerUnsavedEditor } from "../workspace/unsaved-editor-changes";
import { switchWithTerminalConfirm } from "./workspace-switch-confirmation";

vi.mock("../../api/client", () => ({ api: { workspaces: { switch: vi.fn() } } }));

const t: Translate = (en) => en;
const cleanups: Array<() => void> = [];

afterEach(() => {
  cleanups.splice(0).forEach((cleanup) => cleanup());
  vi.resetAllMocks();
});

describe("switchWithTerminalConfirm", () => {
  it("switches clean workspaces without an extra confirmation", async () => {
    const confirm = vi.fn();
    await expect(switchWithTerminalConfirm("target", confirm, t)).resolves.toBe(true);
    expect(api.workspaces.switch).toHaveBeenCalledExactlyOnceWith("target");
    expect(confirm).not.toHaveBeenCalled();
  });

  it("keeps unsaved edits and does not contact the backend when cancelled", async () => {
    cleanups.push(registerUnsavedEditor("src/main.ts"));
    const confirm = vi.fn().mockResolvedValue(false);

    await expect(switchWithTerminalConfirm("target", confirm, t)).resolves.toBe(false);
    expect(api.workspaces.switch).not.toHaveBeenCalled();
    expect(getUnsavedEditorPaths()).toEqual(["src/main.ts"]);
    expect(confirm).toHaveBeenCalledWith(expect.objectContaining({
      description: expect.stringContaining("src/main.ts"),
      danger: true
    }));
  });

  it("waits for approval before requesting a switch with unsaved files", async () => {
    cleanups.push(registerUnsavedEditor("src/main.ts"));
    let answer: (accepted: boolean) => void = () => {};
    const confirm = vi.fn(() => new Promise<boolean>((resolve) => { answer = resolve; }));
    const pending = switchWithTerminalConfirm("target", confirm, t);

    expect(api.workspaces.switch).not.toHaveBeenCalled();
    answer(true);
    await expect(pending).resolves.toBe(true);
    expect(api.workspaces.switch).toHaveBeenCalledExactlyOnceWith("target");
  });

  it("keeps a file protected until its last unsaved editor is cleared", async () => {
    const first = registerUnsavedEditor("src/main.ts");
    const second = registerUnsavedEditor("src/main.ts");
    cleanups.push(first, second);
    first();
    const confirm = vi.fn().mockResolvedValue(false);
    await expect(switchWithTerminalConfirm("target", confirm, t)).resolves.toBe(false);
    expect(getUnsavedEditorPaths()).toEqual(["src/main.ts"]);

    second();
    confirm.mockClear();
    await expect(switchWithTerminalConfirm("target", confirm, t)).resolves.toBe(true);
    expect(confirm).not.toHaveBeenCalled();
  });

  it("does not force-close terminals when the terminal confirmation is cancelled", async () => {
    vi.mocked(api.workspaces.switch).mockRejectedValueOnce(new Error("terminal sessions are running"));
    const confirm = vi.fn().mockResolvedValue(false);
    await expect(switchWithTerminalConfirm("target", confirm, t)).resolves.toBe(false);
    expect(api.workspaces.switch).toHaveBeenCalledExactlyOnceWith("target");
  });

  it("retries with terminal closure only after confirmation", async () => {
    vi.mocked(api.workspaces.switch).mockRejectedValueOnce(new Error("terminal sessions are running"));
    const confirm = vi.fn().mockResolvedValue(true);
    await expect(switchWithTerminalConfirm("target", confirm, t)).resolves.toBe(true);
    expect(api.workspaces.switch).toHaveBeenNthCalledWith(1, "target");
    expect(api.workspaces.switch).toHaveBeenNthCalledWith(2, "target", true);
  });

  it("keeps edits when discarding was accepted but terminal closure was cancelled", async () => {
    cleanups.push(registerUnsavedEditor("src/main.ts"));
    vi.mocked(api.workspaces.switch).mockRejectedValueOnce(new Error("terminal sessions are running"));
    const confirm = vi.fn().mockResolvedValueOnce(true).mockResolvedValueOnce(false);
    await expect(switchWithTerminalConfirm("target", confirm, t)).resolves.toBe(false);
    expect(api.workspaces.switch).toHaveBeenCalledExactlyOnceWith("target");
    expect(getUnsavedEditorPaths()).toEqual(["src/main.ts"]);
  });

  it("propagates unrelated backend failures without suggesting terminal closure", async () => {
    const failure = new Error("workspace does not exist");
    vi.mocked(api.workspaces.switch).mockRejectedValueOnce(failure);
    const confirm = vi.fn();
    await expect(switchWithTerminalConfirm("target", confirm, t)).rejects.toBe(failure);
    expect(confirm).not.toHaveBeenCalled();
  });
});
