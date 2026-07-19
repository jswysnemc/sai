export type GitPatchLineKind = "context" | "added" | "removed";

export type GitPatchSelectionLine = {
  id: number;
  kind: GitPatchLineKind;
  text: string;
  oldLine?: number;
  newLine?: number;
};

export type GitSelectablePatchHunk = {
  lines: GitPatchSelectionLine[];
  changedLineIds: number[];
};

export type GitPatchApplicationDirection = "forward" | "reverse";

type ParsedPatchHunk = GitSelectablePatchHunk & {
  fileHeader: string[];
  oldStart: number;
  newStart: number;
  headerSuffix: string;
};

const HUNK_HEADER = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(.*)$/;
const UNSUPPORTED_FILE_HEADER = /^(new file mode|deleted file mode|rename from|rename to|copy from|copy to|Binary files |GIT binary patch)/;

/**
 * 解析允许选中单行的普通文本修改区块。
 *
 * @param patch 包含单个 hunk 的 unified patch
 * @returns 可选择行及行号；新建、删除、重命名和特殊补丁返回空
 */
export function parseSelectableGitPatchHunk(patch: string): GitSelectablePatchHunk | null {
  const parsed = parsePatchHunk(patch);
  if (!parsed) return null;
  return { lines: parsed.lines, changedLineIds: parsed.changedLineIds };
}

/**
 * 根据选中的增删行创建可交给 git apply 的最小补丁。
 *
 * @param patch 包含单个 hunk 的 unified patch
 * @param selectedLineIds 待操作行的编号集合
 * @param direction 正向应用或反向应用
 * @returns 重新计算行数的 unified patch；选择无效时返回空
 */
export function buildSelectedGitPatch(
  patch: string,
  selectedLineIds: ReadonlySet<number>,
  direction: GitPatchApplicationDirection
): string | null {
  const parsed = parsePatchHunk(patch);
  if (!parsed) return null;
  const changedIds = new Set(parsed.changedLineIds);
  const selected = new Set([...selectedLineIds].filter((id) => changedIds.has(id)));
  if (selected.size === 0) return null;

  // 1. 未选改动转换为目标版本中的上下文或直接移除
  const body: string[] = [];
  for (const line of parsed.lines) {
    if (line.kind === "context") {
      body.push(` ${line.text}`);
    } else if (line.kind === "removed") {
      if (selected.has(line.id)) body.push(`-${line.text}`);
      else if (direction === "forward") body.push(` ${line.text}`);
    } else if (selected.has(line.id)) {
      body.push(`+${line.text}`);
    } else if (direction === "reverse") {
      body.push(` ${line.text}`);
    }
  }

  // 2. 新旧行数必须匹配转换后的补丁，而不是原始 hunk
  const oldCount = body.filter((line) => !line.startsWith("+")).length;
  const newCount = body.filter((line) => !line.startsWith("-")).length;
  const hunkHeader = `@@ -${formatRange(parsed.oldStart, oldCount)} +${formatRange(parsed.newStart, newCount)} @@${parsed.headerSuffix}`;
  return [...parsed.fileHeader, hunkHeader, ...body].join("\n").replace(/\n*$/, "\n");
}

/**
 * 解析单个标准 unified hunk，并计算新旧行号。
 *
 * @param patch 待解析补丁
 * @returns 补丁头、范围与可选择行
 */
function parsePatchHunk(patch: string): ParsedPatchHunk | null {
  const lines = patch.replaceAll("\r\n", "\n").replace(/\n$/, "").split("\n");
  const hunkIndex = lines.findIndex((line) => line.startsWith("@@"));
  if (hunkIndex <= 0 || lines.slice(hunkIndex + 1).some((line) => line.startsWith("@@"))) return null;
  const fileHeader = lines.slice(0, hunkIndex);
  if (fileHeader.some((line) => UNSUPPORTED_FILE_HEADER.test(line))) return null;
  const match = HUNK_HEADER.exec(lines[hunkIndex]);
  if (!match) return null;

  let oldLine = Number(match[1]);
  let newLine = Number(match[3]);
  const parsedLines: GitPatchSelectionLine[] = [];
  const changedLineIds: number[] = [];
  for (const [bodyIndex, rawLine] of lines.slice(hunkIndex + 1).entries()) {
    const marker = rawLine[0];
    if (!marker || ![" ", "+", "-"].includes(marker)) return null;
    const id = bodyIndex;
    const text = rawLine.slice(1);
    if (marker === " ") {
      parsedLines.push({ id, kind: "context", text, oldLine, newLine });
      oldLine += 1;
      newLine += 1;
    } else if (marker === "-") {
      parsedLines.push({ id, kind: "removed", text, oldLine });
      changedLineIds.push(id);
      oldLine += 1;
    } else {
      parsedLines.push({ id, kind: "added", text, newLine });
      changedLineIds.push(id);
      newLine += 1;
    }
  }
  if (changedLineIds.length === 0) return null;
  return {
    fileHeader,
    oldStart: Number(match[1]),
    newStart: Number(match[3]),
    headerSuffix: match[5],
    lines: parsedLines,
    changedLineIds
  };
}

/**
 * 格式化 unified diff 范围。
 *
 * @param start 起始行
 * @param count 行数
 * @returns Git hunk 范围文本
 */
function formatRange(start: number, count: number): string {
  return count === 1 ? `${start}` : `${start},${count}`;
}
