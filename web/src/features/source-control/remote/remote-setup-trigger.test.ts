import { describe, expect, it } from "vitest";
import { isMissingRemoteError, isRemoteDependentAction, shouldPromptRemoteSetup } from "./remote-setup-trigger";

describe("isRemoteDependentAction", () => {
  it("accepts the actions that need a remote", () => {
    expect(isRemoteDependentAction("fetch")).toBe(true);
    expect(isRemoteDependentAction("pull_rebase")).toBe(true);
    expect(isRemoteDependentAction("sync")).toBe(true);
  });

  it("rejects local-only actions", () => {
    expect(isRemoteDependentAction("commit")).toBe(false);
    expect(isRemoteDependentAction("stage")).toBe(false);
  });
});

describe("isMissingRemoteError", () => {
  it("matches the backend messages for a missing remote", () => {
    expect(isMissingRemoteError("repository has no remote configured")).toBe(true);
    expect(isMissingRemoteError("current branch has no upstream and origin remote is unavailable")).toBe(true);
    expect(isMissingRemoteError("remote does not exist: upstream")).toBe(true);
  });

  it("ignores case differences", () => {
    expect(isMissingRemoteError("Repository Has No Remote Configured")).toBe(true);
  });

  it("does not match unrelated failures", () => {
    expect(isMissingRemoteError("could not read from remote repository: permission denied")).toBe(false);
    expect(isMissingRemoteError("failed to connect to github.com port 443")).toBe(false);
    expect(isMissingRemoteError("")).toBe(false);
  });
});

describe("shouldPromptRemoteSetup", () => {
  it("prompts when a remote action fails for a missing remote", () => {
    expect(shouldPromptRemoteSetup("push", "repository has no remote configured")).toBe(true);
  });

  it("stays silent when the action does not need a remote", () => {
    expect(shouldPromptRemoteSetup("commit", "repository has no remote configured")).toBe(false);
  });

  it("stays silent when the failure is unrelated to the remote setup", () => {
    expect(shouldPromptRemoteSetup("push", "authentication failed")).toBe(false);
  });
});
