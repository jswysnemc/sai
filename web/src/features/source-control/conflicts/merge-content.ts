type ConflictSection = "common" | "ours" | "base" | "theirs";

const START_MARKER = /^<{7,}(?: .*)?\r?\n?$/;
const BASE_MARKER = /^\|{7,}(?: .*)?\r?\n?$/;
const SEPARATOR_MARKER = /^={7,}\r?\n?$/;
const END_MARKER = /^>{7,}(?: .*)?\r?\n?$/;

/**
 * 按 Git 冲突标记合并每个冲突块，同时只保留一份公共内容。
 *
 * @param current 包含 Git 冲突标记的当前工作树文本
 * @returns 合并后的文本；没有完整冲突块时返回 null
 */
export function combineConflictBlocks(current: string): string | null {
  const lines = current.match(/[^\n]*\n|[^\n]+$/g) ?? [];
  const result: string[] = [];
  let section: ConflictSection = "common";
  let ours: string[] = [];
  let theirs: string[] = [];
  let conflictCount = 0;

  for (const line of lines) {
    if (section === "common" && START_MARKER.test(line)) {
      section = "ours";
      ours = [];
      theirs = [];
      continue;
    }
    if (section === "ours" && BASE_MARKER.test(line)) {
      section = "base";
      continue;
    }
    if ((section === "ours" || section === "base") && SEPARATOR_MARKER.test(line)) {
      section = "theirs";
      continue;
    }
    if (section === "theirs" && END_MARKER.test(line)) {
      result.push(joinConflictSides(ours.join(""), theirs.join("")));
      section = "common";
      conflictCount += 1;
      continue;
    }

    if (section === "common") result.push(line);
    else if (section === "ours") ours.push(line);
    else if (section === "theirs") theirs.push(line);
  }

  return section === "common" && conflictCount > 0 ? result.join("") : null;
}

/**
 * 拼接单个冲突块的双方内容，并避免两侧内容粘连。
 *
 * @param ours 当前分支冲突块
 * @param theirs 合入分支冲突块
 * @returns 顺序拼接后的冲突块
 */
function joinConflictSides(ours: string, theirs: string): string {
  if (!ours) return theirs;
  if (!theirs) return ours;
  return `${ours}${ours.endsWith("\n") ? "" : "\n"}${theirs}`;
}
