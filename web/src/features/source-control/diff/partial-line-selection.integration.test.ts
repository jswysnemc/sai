import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { splitGitPatchHunks } from "./partial-diff";
import { buildSelectedGitPatch, parseSelectableGitPatchHunk } from "./partial-line-selection";

describe("selected line patches with system Git", () => {
  let repository = "";

  beforeEach(() => {
    repository = mkdtempSync(join(tmpdir(), "sai-selected-lines-"));
    git(["init", "-b", "main"]);
    git(["config", "user.name", "Sai Test"]);
    git(["config", "user.email", "sai@example.com"]);
    writeFileSync(filePath(), "alpha\nold\nmiddle\nlast\n");
    git(["add", "file.txt"]);
    git(["commit", "-m", "initial"]);
  });

  afterEach(() => {
    rmSync(repository, { recursive: true, force: true });
  });

  it("stages, unstages, and discards only selected changed lines", () => {
    writeFileSync(filePath(), "alpha\nnew\nmiddle\nfinal\n");
    const workingPatch = git(["diff", "--", "file.txt"]);
    const firstChange = selectChangePair(workingPatch, "old", "new", "forward");

    // 1. 暂存第一组改动，第二组仍只存在于工作树
    git(["apply", "--cached", "--recount", "--whitespace=nowarn", "-"], firstChange);
    expect(git(["show", ":file.txt"])).toBe("alpha\nnew\nmiddle\nlast\n");
    expect(readFileSync(filePath(), "utf8")).toBe("alpha\nnew\nmiddle\nfinal\n");

    // 2. 反向应用已暂存补丁，只取消暂存第一组改动
    const stagedPatch = git(["diff", "--cached", "--", "file.txt"]);
    const stagedChange = selectChangePair(stagedPatch, "old", "new", "reverse");
    git(["apply", "--cached", "--reverse", "--recount", "--whitespace=nowarn", "-"], stagedChange);
    expect(git(["show", ":file.txt"])).toBe("alpha\nold\nmiddle\nlast\n");

    // 3. 再次暂存第一组并反向应用第二组，工作树最终与暂存区一致
    git(["apply", "--cached", "--recount", "--whitespace=nowarn", "-"], firstChange);
    const remainingPatch = git(["diff", "--", "file.txt"]);
    const secondChange = selectChangePair(remainingPatch, "last", "final", "reverse");
    git(["apply", "--reverse", "--recount", "--whitespace=nowarn", "-"], secondChange);
    expect(readFileSync(filePath(), "utf8")).toBe("alpha\nnew\nmiddle\nlast\n");
    expect(git(["diff", "--", "file.txt"])).toBe("");
  });

  it("unstages a later change while retaining an earlier staged change", () => {
    writeFileSync(filePath(), "alpha\nnew\nmiddle\nfinal\n");
    git(["add", "file.txt"]);
    const stagedPatch = git(["diff", "--cached", "--", "file.txt"]);
    const secondChange = selectChangePair(stagedPatch, "last", "final", "reverse");

    git(["apply", "--cached", "--reverse", "--recount", "--whitespace=nowarn", "-"], secondChange);

    expect(git(["show", ":file.txt"])).toBe("alpha\nnew\nmiddle\nlast\n");
    expect(readFileSync(filePath(), "utf8")).toBe("alpha\nnew\nmiddle\nfinal\n");
  });

  it("discards a later change while retaining an earlier working change", () => {
    writeFileSync(filePath(), "alpha\nnew\nmiddle\nfinal\n");
    const workingPatch = git(["diff", "--", "file.txt"]);
    const secondChange = selectChangePair(workingPatch, "last", "final", "reverse");

    git(["apply", "--reverse", "--recount", "--whitespace=nowarn", "-"], secondChange);

    expect(readFileSync(filePath(), "utf8")).toBe("alpha\nnew\nmiddle\nlast\n");
    expect(git(["diff", "--", "file.txt"])).toContain("+new");
  });

  /**
   * 执行当前临时仓库中的系统 Git 命令。
   *
   * @param args Git 参数
   * @param input 可选标准输入补丁
   * @returns Git 标准输出
   */
  function git(args: string[], input?: string): string {
    return execFileSync("git", args, {
      cwd: repository,
      encoding: "utf8",
      input,
      env: { ...process.env, LC_ALL: "C", GIT_CONFIG_NOSYSTEM: "1" }
    });
  }

  /**
   * 返回测试文件绝对路径。
   *
   * @returns 临时仓库中的测试文件路径
   */
  function filePath(): string {
    return join(repository, "file.txt");
  }
});

/**
 * 从单个 Git diff 中选择一组删除与新增行并生成补丁。
 *
 * @param patch Git unified diff
 * @param removedText 待选择删除行文本
 * @param addedText 待选择新增行文本
 * @param direction 补丁应用方向
 * @returns 可交给系统 Git 的选中行补丁
 */
function selectChangePair(
  patch: string,
  removedText: string,
  addedText: string,
  direction: "forward" | "reverse"
): string {
  const hunk = splitGitPatchHunks(patch)[0];
  const parsed = hunk && parseSelectableGitPatchHunk(hunk.patch);
  if (!hunk || !parsed) throw new Error("expected a selectable Git hunk");
  const selected = new Set(parsed.lines
    .filter((line) => line.text === removedText || line.text === addedText)
    .map((line) => line.id));
  const result = buildSelectedGitPatch(hunk.patch, selected, direction);
  if (!result) throw new Error("expected a selected line patch");
  return result;
}
