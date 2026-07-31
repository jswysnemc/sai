import { annotateInlineDiff } from "./inline-diff";
import type { DiffFile, DiffFileStatus, DiffLine } from "./diff-model";

const CODEX_FILE = /^\*\*\* (Add|Delete|Update) File: (.+)$/;
const UNIFIED_HUNK = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;
const CODEX_RANGE_HUNK = /^@@ (?:(?:第|Lines?) )?(\d+)(?:-\d+)?(?: 行| lines?)?/i;

/** 解析器所处的位置：文件头区域或 hunk 正文 */
type ParserState = "header" | "hunk";

/**
 * 把 Codex patch 或 unified diff 文本解析为文件块结构。
 *
 * 关键在于按位置分类而不是按前缀猜测：只有处在 hunk 正文时，首列字符才决定
 * 行的类型。旧实现用 `startsWith("--- ")` 一类判断跳过文件头，会把内容为
 * `-- note` 的删除行、`++i` 的新增行误当成头部丢弃。
 *
 * @param source Diff 源文本
 * @returns 带行号、状态与字符级差异的文件块列表
 */
export function parseDiff(source: string): DiffFile[] {
  const lines = source.replaceAll("\r\n", "\n").split("\n");
  const files: DiffFile[] = [];
  let current: DiffFile | undefined;
  let state: ParserState = "header";
  let oldNumber: number | null = null;
  let newNumber: number | null = null;

  /**
   * 开启新文件块并重置行号计数。
   *
   * Codex patch 的内容行紧跟文件头，中间可能没有 `@@`，因此这类文件头之后
   * 直接进入正文状态；unified diff 还要先读完 index/mode 等元信息。
   */
  const openFile = (path: string, status: DiffFileStatus, startAtOne: boolean): DiffFile => {
    const file: DiffFile = { path, status, added: 0, removed: 0, lines: [] };
    current = file;
    files.push(file);
    state = startAtOne ? "hunk" : "header";
    oldNumber = startAtOne ? 1 : null;
    newNumber = startAtOne ? 1 : null;
    return file;
  };

  for (const line of lines) {
    // 1. 文件起始标记在任何状态下都要识别：多文件补丁里它紧跟上一个文件的正文
    const codexHead = CODEX_FILE.exec(line);
    if (codexHead) {
      const status: DiffFileStatus =
        codexHead[1] === "Add" ? "added" : codexHead[1] === "Delete" ? "deleted" : "modified";
      openFile(codexHead[2].trim(), status, true);
      continue;
    }
    if (line.startsWith("diff --git ")) {
      openFile(parseGitHeaderPath(line), "modified", false);
      continue;
    }

    // 2. 其余文件头只在 header 状态识别，避免吃掉正文
    if (state === "header") {
      if (current && consumeFileMetadata(current, line)) continue;
      if (line.startsWith("+++ ")) {
        const path = stripPathPrefix(line.slice(4));
        const active = current;
        if (!active) {
          openFile(path, "modified", false);
        } else if (path !== "/dev/null" && (!active.path || active.path === "/dev/null")) {
          active.path = path;
        }
        continue;
      }
      if (
        line.startsWith("*** Begin Patch") ||
        line.startsWith("*** End Patch") ||
        line.startsWith("--- ") ||
        line.startsWith("*** Move to:")
      ) {
        continue;
      }
    }

    // 3. hunk 头：进入正文状态并同步行号
    if (line.startsWith("@@")) {
      const file = current ?? openFile("", "modified", true);
      const hunk = UNIFIED_HUNK.exec(line);
      if (hunk) {
        oldNumber = Number(hunk[1]);
        newNumber = Number(hunk[2]);
      } else {
        const range = CODEX_RANGE_HUNK.exec(line);
        if (range) {
          oldNumber = Number(range[1]);
          newNumber = Number(range[1]);
        } else {
          oldNumber ??= 1;
          newNumber ??= 1;
        }
      }
      // 保留 hunk 边界，否则不相邻的区段会连在一起看不出跳过了内容
      if (file.lines.length > 0) file.lines.push({ kind: "hunk", text: line });
      state = "hunk";
      continue;
    }

    if (!current && !line.trim()) continue;
    const file = current ?? openFile("", "modified", true);

    // 4. 无换行标记不是内容，不能占用行号
    if (line.startsWith("\\")) {
      file.lines.push({ kind: "no-newline", text: line.slice(1).trim() });
      continue;
    }

    // 5. hunk 正文按首列分类；header 状态下的残余行按上下文处理
    const marker = state === "hunk" ? line.charAt(0) : " ";
    if (marker === "+") {
      file.added += 1;
      file.lines.push({ kind: "added", text: line.slice(1), newLine: newNumber ?? undefined });
      if (newNumber !== null) newNumber += 1;
      continue;
    }
    if (marker === "-") {
      file.removed += 1;
      file.lines.push({ kind: "removed", text: line.slice(1), oldLine: oldNumber ?? undefined });
      if (oldNumber !== null) oldNumber += 1;
      continue;
    }
    file.lines.push({
      kind: "context",
      text: state === "hunk" ? line.slice(1) : line,
      oldLine: oldNumber ?? undefined,
      newLine: newNumber ?? undefined
    });
    if (oldNumber !== null) oldNumber += 1;
    if (newNumber !== null) newNumber += 1;
  }

  return files
    .filter((file) => file.lines.length > 0 || file.status !== "modified")
    .map((file) => ({ ...file, lines: annotateInlineDiff(file.lines) }));
}

/**
 * 识别并吸收文件级元信息行。
 *
 * 这些行描述的是文件本身而不是内容，渲染成代码行会造成误导；旧实现只跳过
 * `index`，其余（新建、删除、重命名、二进制）都被当成上下文渲染。
 *
 * @param file 当前文件块
 * @param line 待判定的行
 * @returns 已作为元信息消费时返回 true
 */
function consumeFileMetadata(file: DiffFile, line: string): boolean {
  if (line.startsWith("index ") || line.startsWith("old mode ")) return true;
  if (line.startsWith("new file mode")) {
    file.status = "added";
    return true;
  }
  if (line.startsWith("deleted file mode")) {
    file.status = "deleted";
    return true;
  }
  if (line.startsWith("new mode ")) {
    if (file.status === "modified") file.status = "mode-changed";
    return true;
  }
  if (line.startsWith("similarity index ")) return true;
  if (line.startsWith("rename from ")) {
    file.oldPath = line.slice("rename from ".length).trim();
    file.status = "renamed";
    return true;
  }
  if (line.startsWith("rename to ")) {
    file.path = line.slice("rename to ".length).trim();
    file.status = "renamed";
    return true;
  }
  if (line.startsWith("Binary files ") || line.startsWith("GIT binary patch")) {
    file.status = "binary";
    return true;
  }
  return false;
}

/**
 * 从 `diff --git` 头解析目标路径。
 *
 * 直接按 `" b/"` 切分会截断名字里含该串的文件；这里按前后两段等长的特性
 * 从中点拆分，失败时再回落到分隔符匹配。
 *
 * @param line `diff --git` 行
 * @returns 目标路径
 */
function parseGitHeaderPath(line: string): string {
  const body = line.slice("diff --git ".length).trim();
  // a/<path> b/<path>：两侧路径相同，长度可用于精确定位分隔点
  if (body.length % 2 === 1) {
    const half = (body.length - 1) / 2;
    const left = body.slice(0, half);
    const right = body.slice(half + 1);
    if (left.startsWith("a/") && right.startsWith("b/") && left.slice(2) === right.slice(2)) {
      return right.slice(2);
    }
  }
  const separator = body.lastIndexOf(" b/");
  return separator >= 0 ? body.slice(separator + 3) : body;
}

/**
 * 去掉 `a/` 或 `b/` 前缀。
 *
 * @param path 原始路径
 * @returns 去前缀后的路径
 */
function stripPathPrefix(path: string): string {
  return path.trim().replace(/^[ab]\//, "");
}

export type { DiffFile, DiffLine, DiffFileStatus } from "./diff-model";
