import { describe, expect, it } from "vitest";
import { isReadOnlyShellCommand } from "./read-only-command";

describe("read-only shell command detection", () => {
  it("accepts plain read commands", () => {
    expect(isReadOnlyShellCommand("cat src/main.rs")).toBe(true);
    expect(isReadOnlyShellCommand("ls -la web/src")).toBe(true);
    expect(isReadOnlyShellCommand("rg 'toolCallGroupLabel' web/src")).toBe(true);
  });

  it("accepts read-only pipelines and chained reads", () => {
    expect(isReadOnlyShellCommand("cat a.log | grep error | wc -l")).toBe(true);
    expect(isReadOnlyShellCommand("cd web && ls src")).toBe(true);
    expect(isReadOnlyShellCommand("head -n 5 a.txt; tail -n 5 a.txt")).toBe(true);
  });

  it("accepts read-only git subcommands", () => {
    expect(isReadOnlyShellCommand("git status")).toBe(true);
    expect(isReadOnlyShellCommand("git log --oneline -5")).toBe(true);
    expect(isReadOnlyShellCommand("git -C web diff --stat")).toBe(true);
  });

  it("rejects mutating git subcommands", () => {
    expect(isReadOnlyShellCommand("git commit -m 'x'")).toBe(false);
    expect(isReadOnlyShellCommand("git push origin main")).toBe(false);
    expect(isReadOnlyShellCommand("git checkout -b feat")).toBe(false);
  });

  it("rejects any segment outside the allowlist", () => {
    expect(isReadOnlyShellCommand("cargo test")).toBe(false);
    expect(isReadOnlyShellCommand("cat a.txt && rm a.txt")).toBe(false);
    expect(isReadOnlyShellCommand("find . -name '*.tmp' | xargs rm")).toBe(false);
  });

  it("rejects output redirection but tolerates harmless ones", () => {
    expect(isReadOnlyShellCommand("cat a.txt > b.txt")).toBe(false);
    expect(isReadOnlyShellCommand("echo hi >> notes.md")).toBe(false);
    expect(isReadOnlyShellCommand("ls missing 2>/dev/null")).toBe(true);
    expect(isReadOnlyShellCommand("grep -r foo . 2>&1")).toBe(true);
  });

  it("skips env assignment prefixes and absolute paths", () => {
    expect(isReadOnlyShellCommand("LANG=C ls src")).toBe(true);
    expect(isReadOnlyShellCommand("/usr/bin/cat notes.md")).toBe(true);
  });

  it("rejects empty and unknown commands", () => {
    expect(isReadOnlyShellCommand("")).toBe(false);
    expect(isReadOnlyShellCommand("   ")).toBe(false);
    expect(isReadOnlyShellCommand("sudo cat /etc/shadow")).toBe(false);
  });
});
