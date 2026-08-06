import { describe, expect, it } from "vitest";
import { parseDiff } from "./diff-parser";

describe("diff parser", () => {
  it("keeps deleted lines that start with dashes", () => {
    // 内容为 `-- legacy note` 的删除行在补丁里是 `--- legacy note`，
    // 按前缀猜测会把它当成 `--- a/file` 文件头丢弃
    const files = parseDiff(
      ["diff --git a/q.sql b/q.sql", "--- a/q.sql", "+++ b/q.sql", "@@ -1,2 +1,1 @@", " SELECT 1;", "--- legacy note"].join("\n")
    );

    expect(files[0].removed).toBe(1);
    expect(files[0].lines.some((line) => line.kind === "removed" && line.text === "-- legacy note")).toBe(true);
  });

  it("keeps added lines that start with plus signs", () => {
    const files = parseDiff(
      ["diff --git a/m.c b/m.c", "--- a/m.c", "+++ b/m.c", "@@ -1,1 +1,2 @@", " int i;", "+++i;"].join("\n")
    );

    expect(files[0].added).toBe(1);
    expect(files[0].lines.some((line) => line.kind === "added" && line.text === "++i;")).toBe(true);
  });

  it("does not let the no-newline marker consume line numbers", () => {
    const files = parseDiff(
      [
        "diff --git a/a.txt b/a.txt",
        "--- a/a.txt",
        "+++ b/a.txt",
        "@@ -1,2 +1,2 @@",
        " keep",
        "-old",
        "\\ No newline at end of file",
        "+new",
        "\\ No newline at end of file"
      ].join("\n")
    );

    const numbered = files[0].lines.filter((line) => line.kind !== "no-newline" && line.kind !== "hunk");
    // 文件只有两行，行号不能被标记撑到 4
    expect(Math.max(...numbered.map((line) => line.oldLine ?? 0))).toBe(2);
    expect(Math.max(...numbered.map((line) => line.newLine ?? 0))).toBe(2);
    expect(files[0].lines.filter((line) => line.kind === "no-newline")).toHaveLength(2);
  });

  it("reports file level status instead of rendering metadata as code", () => {
    const deleted = parseDiff(
      ["diff --git a/gone.txt b/gone.txt", "deleted file mode 100644", "--- a/gone.txt", "+++ /dev/null", "@@ -1 +0,0 @@", "-bye"].join("\n")
    );
    expect(deleted[0].status).toBe("deleted");
    expect(deleted[0].path).toBe("gone.txt");
    expect(deleted[0].lines.some((line) => line.text.includes("deleted file mode"))).toBe(false);

    const added = parseDiff(
      ["diff --git a/new.txt b/new.txt", "new file mode 100644", "--- /dev/null", "+++ b/new.txt", "@@ -0,0 +1 @@", "+hi"].join("\n")
    );
    expect(added[0].status).toBe("added");
  });

  it("recognizes renames and binary files", () => {
    const renamed = parseDiff(
      ["diff --git a/old.txt b/new.txt", "similarity index 100%", "rename from old.txt", "rename to new.txt"].join("\n")
    );
    expect(renamed[0].status).toBe("renamed");
    expect(renamed[0].oldPath).toBe("old.txt");
    expect(renamed[0].path).toBe("new.txt");
    expect(renamed[0].lines).toHaveLength(0);

    const binary = parseDiff(
      ["diff --git a/logo.png b/logo.png", "index 111..222 100644", "Binary files a/logo.png and b/logo.png differ"].join("\n")
    );
    expect(binary[0].status).toBe("binary");
    expect(binary[0].lines).toHaveLength(0);
  });

  it("keeps paths that contain the b/ separator", () => {
    const files = parseDiff(
      ["diff --git a/my b/dir.txt b/my b/dir.txt", "--- a/my b/dir.txt", "+++ b/my b/dir.txt", "@@ -1 +1 @@", "-x", "+y"].join("\n")
    );

    expect(files[0].path).toBe("my b/dir.txt");
  });

  it("keeps trailing blank context lines", () => {
    const files = parseDiff(
      ["diff --git a/t.txt b/t.txt", "--- a/t.txt", "+++ b/t.txt", "@@ -1,3 +1,3 @@", "-a", "+b", " ", " "].join("\n")
    );

    // 空白上下文行是真实内容，删掉会让读者以为文件到此为止
    const context = files[0].lines.filter((line) => line.kind === "context");
    expect(context).toHaveLength(2);
  });

  it("marks hunk boundaries so skipped regions stay visible", () => {
    const files = parseDiff(
      [
        "diff --git a/h.txt b/h.txt",
        "--- a/h.txt",
        "+++ b/h.txt",
        "@@ -1,1 +1,1 @@",
        "-one",
        "+ONE",
        "@@ -50,1 +50,1 @@",
        "-fifty",
        "+FIFTY"
      ].join("\n")
    );

    expect(files[0].lines.filter((line) => line.kind === "hunk")).toHaveLength(1);
    const second = files[0].lines.find((line) => line.kind === "removed" && line.text === "fifty");
    expect(second?.oldLine).toBe(50);
  });

  it("pairs adjacent changes with character level segments", () => {
    const files = parseDiff(
      ["diff --git a/p.ts b/p.ts", "--- a/p.ts", "+++ b/p.ts", "@@ -1 +1 @@", "-const a = 1;", "+const a = 2;"].join("\n")
    );

    const removed = files[0].lines.find((line) => line.kind === "removed");
    const added = files[0].lines.find((line) => line.kind === "added");
    // 只改了一个字符，就只应高亮那一个字符
    expect(removed?.segments?.filter((item) => item.changed).map((item) => item.text)).toEqual(["1"]);
    expect(added?.segments?.filter((item) => item.changed).map((item) => item.text)).toEqual(["2"]);
  });

  it("keeps codex patch headers working", () => {
    const files = parseDiff(
      ["*** Begin Patch", "*** Update File: src/main.rs", "@@ -1 +1 @@", "-old", "+new", "*** End Patch"].join("\n")
    );

    expect(files[0].path).toBe("src/main.rs");
    expect(files[0].status).toBe("modified");
    expect(files[0].added).toBe(1);
    expect(files[0].removed).toBe(1);
    expect(files[0].lines.some((line) => line.text.includes("End Patch"))).toBe(false);
  });

  it("assigns line numbers across multiple hunks", () => {
    const files = parseDiff(
      [
        "diff --git a/src/a.ts b/src/a.ts",
        "--- a/src/a.ts",
        "+++ b/src/a.ts",
        "@@ -2,2 +2,2 @@",
        " const a = 1;",
        "-oldValue();",
        "+newValue();",
        "@@ -10 +10 @@",
        "-before();",
        "+after();"
      ].join("\n")
    );

    expect(files).toHaveLength(1);
    expect(files[0].lines.find((line) => line.text === "oldValue();")).toMatchObject({ oldLine: 3 });
    expect(files[0].lines.find((line) => line.text === "newValue();")).toMatchObject({ newLine: 3 });
    expect(files[0].lines.find((line) => line.text === "before();")).toMatchObject({ oldLine: 10 });
    expect(files[0].lines.find((line) => line.text === "after();")).toMatchObject({ newLine: 10 });
  });

  it("splits codex patches into files with their own counters", () => {
    const files = parseDiff(
      [
        "*** Begin Patch",
        "*** Add File: src/new.ts",
        "+export const value = 1;",
        "*** Delete File: src/old.ts",
        "-export const old = true;",
        "*** End Patch"
      ].join("\n")
    );

    expect(files.map((file) => [file.path, file.status, file.added, file.removed])).toEqual([
      ["src/new.ts", "added", 1, 0],
      ["src/old.ts", "deleted", 0, 1]
    ]);
  });

  it("numbers codex updates that omit hunk ranges", () => {
    const files = parseDiff(
      ["*** Update File: src/a.ts", "@@", "-before();", "+after();", " context();"].join("\n")
    );

    expect(files[0].lines.find((line) => line.text === "before();")).toMatchObject({ oldLine: 1 });
    expect(files[0].lines.find((line) => line.text === "after();")).toMatchObject({ newLine: 1 });
    expect(files[0].lines.find((line) => line.text === "context();")).toMatchObject({
      oldLine: 2,
      newLine: 2
    });
  });

  it("normalizes CRLF input", () => {
    const files = parseDiff("*** Add File: a.txt\r\n+one\r\n");

    expect(files[0].lines[0]).toMatchObject({ kind: "added", text: "one" });
  });

});
