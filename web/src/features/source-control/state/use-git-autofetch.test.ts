import { describe, expect, it } from "vitest";
import { canRunGitAutofetch } from "./use-git-autofetch";

describe("canRunGitAutofetch", () => {
  const ready = {
    enabled: true,
    ready: true,
    remoteConfigured: true,
    busy: false,
    hasOperation: false,
    pageVisible: true,
    online: true
  };

  it("allows an idle visible repository with a remote", () => {
    expect(canRunGitAutofetch(ready)).toBe(true);
  });

  it("pauses for hidden, offline, busy, or in-progress repositories", () => {
    expect(canRunGitAutofetch({ ...ready, pageVisible: false })).toBe(false);
    expect(canRunGitAutofetch({ ...ready, online: false })).toBe(false);
    expect(canRunGitAutofetch({ ...ready, busy: true })).toBe(false);
    expect(canRunGitAutofetch({ ...ready, hasOperation: true })).toBe(false);
    expect(canRunGitAutofetch({ ...ready, remoteConfigured: false })).toBe(false);
  });
});
