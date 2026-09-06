import type { DiffLine } from "./diff-model";

export type DiffCodeBlock = {
  kind: "context" | "change" | "marker";
  lines: DiffLine[];
};

/**
 * 将补丁拆为上下文、变更与边界块，供两种布局共用折叠和导航规则。
 * @param lines 已解析的文件差异行
 * @returns 顺序与补丁一致的代码块
 */
export function buildDiffCodeBlocks(lines: DiffLine[]): DiffCodeBlock[] {
  const blocks: DiffCodeBlock[] = [];
  for (const line of lines) {
    const kind = line.kind === "context" ? "context"
      : line.kind === "hunk" || line.kind === "no-newline" ? "marker" : "change";
    const previous = blocks.at(-1);
    if (kind !== "marker" && previous?.kind === kind) previous.lines.push(line);
    else blocks.push({ kind, lines: [line] });
  }
  return blocks;
}
