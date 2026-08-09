import { describe, expect, it } from "vitest";
import type { SshHost } from "../../../api/contracts";
import {
  DEFAULT_SSH_PORT,
  EMPTY_SSH_HOST_FORM,
  canSubmitSshHostForm,
  sshHostAddress,
  toSshHostForm,
  toSshHostInput,
  validateSshHostForm,
  type SshHostFormState
} from "./ssh-host-form-state";

/**
 * 构造测试用表单编辑态。
 *
 * @param overrides 待覆盖字段
 * @returns 完整表单编辑态
 */
function form(overrides: Partial<SshHostFormState> = {}): SshHostFormState {
  return { ...EMPTY_SSH_HOST_FORM, hostname: "example.com", username: "deploy", ...overrides };
}

describe("validateSshHostForm", () => {
  it("accepts a complete form", () => {
    expect(validateSshHostForm(form())).toEqual({});
  });

  it("requires the hostname and username", () => {
    const errors = validateSshHostForm(form({ hostname: "  ", username: "" }));
    expect(errors.hostname).toBe("required");
    expect(errors.username).toBe("required");
  });

  it("allows an empty port and falls back to the default", () => {
    expect(validateSshHostForm(form({ port: "" }))).toEqual({});
    expect(toSshHostInput(form({ port: "" })).port).toBe(DEFAULT_SSH_PORT);
  });

  it("rejects ports outside the valid range", () => {
    expect(validateSshHostForm(form({ port: "0" })).port).toBe("range");
    expect(validateSshHostForm(form({ port: "65536" })).port).toBe("range");
    expect(validateSshHostForm(form({ port: "-1" })).port).toBe("range");
  });

  it("rejects non-integer ports", () => {
    expect(validateSshHostForm(form({ port: "22.5" })).port).toBe("range");
    expect(validateSshHostForm(form({ port: "abc" })).port).toBe("range");
  });

  it("accepts the boundary ports", () => {
    expect(validateSshHostForm(form({ port: "1" }))).toEqual({});
    expect(validateSshHostForm(form({ port: "65535" }))).toEqual({});
  });
});

describe("canSubmitSshHostForm", () => {
  it("blocks submission while a field is invalid", () => {
    expect(canSubmitSshHostForm(form())).toBe(true);
    expect(canSubmitSshHostForm(form({ hostname: "" }))).toBe(false);
  });
});

describe("toSshHostInput", () => {
  it("trims every field", () => {
    const input = toSshHostInput(
      form({ label: "  box  ", hostname: " example.com ", username: " deploy ", identityFile: " ~/.ssh/id ", remoteDirectory: " /srv " })
    );
    expect(input).toEqual({
      label: "box",
      hostname: "example.com",
      port: DEFAULT_SSH_PORT,
      username: "deploy",
      identity_file: "~/.ssh/id",
      remote_directory: "/srv"
    });
  });

  it("falls back to the hostname when the label is blank", () => {
    expect(toSshHostInput(form({ label: "   " })).label).toBe("example.com");
  });

  it("keeps a valid custom port", () => {
    expect(toSshHostInput(form({ port: "2222" })).port).toBe(2222);
  });
});

describe("toSshHostForm", () => {
  it("round-trips a saved host", () => {
    const host: SshHost = {
      id: "h1",
      label: "box",
      hostname: "example.com",
      port: 2222,
      username: "deploy",
      identity_file: "~/.ssh/id_ed25519",
      remote_directory: "/srv/app"
    };
    expect(toSshHostInput(toSshHostForm(host))).toEqual({
      label: "box",
      hostname: "example.com",
      port: 2222,
      username: "deploy",
      identity_file: "~/.ssh/id_ed25519",
      remote_directory: "/srv/app"
    });
  });
});

describe("sshHostAddress", () => {
  it("omits the default port", () => {
    expect(sshHostAddress({ username: "deploy", hostname: "example.com", port: DEFAULT_SSH_PORT })).toBe(
      "deploy@example.com"
    );
  });

  it("keeps a custom port", () => {
    expect(sshHostAddress({ username: "deploy", hostname: "example.com", port: 2222 })).toBe(
      "deploy@example.com:2222"
    );
  });
});
