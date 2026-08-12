import { parseDiff } from "../chat/tool-renderers/diff/diff-parser";

/** 编辑器行的 Git 变更类型。 */
export type EditorGitLineKind = "added" | "modified" | "deleted";

/** 单行 Git 装饰：行号基于当前工作区文件（1 起）。 */
export type EditorGitLine = {
  line: number;
  kind: EditorGitLineKind;
};

/**
 * 把单文件 unified diff 转换为编辑器行号装饰。
 *
 * 与 VS Code 的 gutter 语义一致：纯新增行标 added，删改混合的
 * 区段给新增侧标 modified，纯删除在其后相邻行留一个 deleted 锚点
 * （编辑器里画三角标记）。行号取补丁的新文件侧，直接对应当前缓冲区。
 *
 * @param patch 该文件 HEAD 到工作树的 unified diff
 * @returns 升序且去重的行装饰
 */
export function buildEditorGitLines(patch: string): EditorGitLine[] {
  // 补丁末尾换行会被解析成多余的空上下文行，剥掉以免文件尾删除锚点偏移
  const file = parseDiff(patch.replace(/\r?\n$/u, ""))[0];
  if (!file) return [];

  const result = new Map<number, EditorGitLineKind>();
  let removedCount = 0;
  let addedLines: number[] = [];
  let lastNewLine = 0;

  /**
   * 收拢当前变更区段并落装饰。
   *
   * @param anchor 区段后紧邻行的新行号；区段收在补丁边界时为 null
   */
  const flush = (anchor: number | null) => {
    if (addedLines.length > 0) {
      const kind: EditorGitLineKind = removedCount > 0 ? "modified" : "added";
      for (const line of addedLines) result.set(line, kind);
    } else if (removedCount > 0) {
      const line = anchor ?? Math.max(lastNewLine, 1);
      // 删除锚点不覆盖已有的增改装饰
      if (!result.has(line)) result.set(line, "deleted");
    }
    removedCount = 0;
    addedLines = [];
  };

  for (const line of file.lines) {
    if (line.kind === "added") {
      if (line.newLine !== undefined) {
        addedLines.push(line.newLine);
        lastNewLine = line.newLine;
      }
      continue;
    }
    if (line.kind === "removed") {
      removedCount += 1;
      continue;
    }
    if (line.kind === "context") {
      flush(line.newLine ?? null);
      if (line.newLine !== undefined) lastNewLine = line.newLine;
      continue;
    }
    if (line.kind === "hunk") flush(null);
  }
  flush(null);

  return [...result.entries()]
    .map(([line, kind]) => ({ line, kind }))
    .sort((left, right) => left.line - right.line);
}
