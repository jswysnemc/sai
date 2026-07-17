export type DiffLineKind = "added" | "removed" | "context" | "hunk";

export type DiffLine = {
  kind: DiffLineKind;
  text: string;
  oldLine?: number;
  newLine?: number;
};

export type DiffFile = {
  path: string;
  action: string;
  added: number;
  removed: number;
  lines: DiffLine[];
};

const CODEX_FILE = /^\*\*\* (Add|Delete|Update) File: (.+)$/;
const UNIFIED_HUNK = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;
const CODEX_RANGE_HUNK = /^@@ 第 (\d+)(?:-\d+)? 行/;

/**
 * 把 Codex patch 或 unified diff 文本解析为文件块结构。
 *
 * @param source Diff 源文本
 * @returns 带行号与增删统计的文件块列表
 */
export function parseDiff(source: string): DiffFile[] {
  const lines = source.replaceAll("\r\n", "\n").split("\n");
  const files: DiffFile[] = [];
  let current: DiffFile | null = null;
  let oldNumber: number | null = null;
  let newNumber: number | null = null;

  // 1. 开启新文件块并重置行号计数
  const openFile = (path: string, action: string, startAtOne: boolean) => {
    current = { path, action, added: 0, removed: 0, lines: [] };
    files.push(current);
    oldNumber = startAtOne ? 1 : null;
    newNumber = startAtOne ? 1 : null;
  };

  for (const line of lines) {
    // 2. 识别 Codex patch 文件头
    const codexHead = CODEX_FILE.exec(line);
    if (codexHead) {
      const action = { Add: "新增", Delete: "删除", Update: "修改" }[codexHead[1]] ?? codexHead[1];
      openFile(codexHead[2].trim(), action, true);
      continue;
    }
    // 3. 识别 unified diff 文件头
    if (line.startsWith("diff --git ")) {
      const path = line.split(" b/").pop()?.trim() ?? line;
      openFile(path, "修改", false);
      continue;
    }
    if (line.startsWith("+++ ")) {
      const path = line.slice(4).replace(/^b\//, "").trim();
      if (!current) {
        openFile(path, "修改", false);
      } else {
        const file = current as DiffFile;
        if (!file.path || file.path === "/dev/null") file.path = path;
      }
      continue;
    }
    // 4. 跳过纯元信息行
    if (line.startsWith("*** Begin Patch") || line.startsWith("*** End Patch") || line.startsWith("--- ") || line.startsWith("index ") || line.startsWith("*** Move to:")) {
      continue;
    }
    // 5. 没有任何文件头时把片段归入匿名文件块
    if (!current) {
      if (!line.trim()) continue;
      openFile("", "变更", true);
    }
    const file = current!;
    // 6. 处理 hunk 头并同步行号计数
    if (line.startsWith("@@")) {
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
      continue;
    }
    // 7. 按前缀分类内容行并分配新旧行号
    if (line.startsWith("+")) {
      file.added += 1;
      file.lines.push({ kind: "added", text: line.slice(1), newLine: newNumber ?? undefined });
      if (newNumber !== null) newNumber += 1;
      continue;
    }
    if (line.startsWith("-")) {
      file.removed += 1;
      file.lines.push({ kind: "removed", text: line.slice(1), oldLine: oldNumber ?? undefined });
      if (oldNumber !== null) oldNumber += 1;
      continue;
    }
    file.lines.push({ kind: "context", text: line.startsWith(" ") ? line.slice(1) : line, oldLine: oldNumber ?? undefined, newLine: newNumber ?? undefined });
    if (oldNumber !== null) oldNumber += 1;
    if (newNumber !== null) newNumber += 1;
  }

  // 8. 去除每个文件块尾部空白行
  for (const file of files) {
    while (file.lines.length > 0 && file.lines[file.lines.length - 1].kind === "context" && !file.lines[file.lines.length - 1].text.trim()) {
      file.lines.pop();
    }
  }
  return files.filter((file) => file.lines.length > 0);
}
