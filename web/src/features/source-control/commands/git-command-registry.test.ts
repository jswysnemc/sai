import { describe, expect, it, vi } from "vitest";
import { GIT_OPERATION_ACTIONS } from "../../../api/git-contracts";
import { executeGitCommand, GIT_COMMANDS } from "./git-command-registry";

describe("Git command registry", () => {
  it("keeps command identifiers unique and grouped", () => {
    const commands = [...GIT_COMMANDS.values()];
    expect(commands.length).toBe(46);
    expect(new Set(commands.map((command) => command.id)).size).toBe(commands.length);
    expect(commands.every((command) => command.id.startsWith("git.") && Boolean(command.group))).toBe(true);
  });

  it("registers every backend operation exactly once", () => {
    const actions = [...GIT_COMMANDS.values()].map((command) => command.action).sort();
    expect(actions).toEqual([...GIT_OPERATION_ACTIONS].sort());
  });

  it("maps commands to typed backend actions", async () => {
    const runOperation = vi.fn().mockResolvedValue(undefined);
    await executeGitCommand("git.pullRebase", runOperation, { repo_root: "/repo" });
    expect(runOperation).toHaveBeenCalledWith("pull_rebase", { repo_root: "/repo" });
    await executeGitCommand("git.pushTo", runOperation, { remote_name: "backup" });
    expect(runOperation).toHaveBeenCalledWith("push_to", { remote_name: "backup" });
  });

  it("marks irreversible commands as destructive", () => {
    expect(GIT_COMMANDS.get("git.cleanAll")?.destructive).toBe(true);
    expect(GIT_COMMANDS.get("git.pushForce")?.destructive).toBe(true);
    expect(GIT_COMMANDS.get("git.fetch")?.destructive).toBe(false);
  });
});
