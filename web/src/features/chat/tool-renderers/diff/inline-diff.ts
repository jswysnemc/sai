import type { DiffLine, DiffSegment } from "./diff-model";

/**
 * 将文本切分为字素簇。
 *
 * emoji 是代理对、带音标的字母是组合序列，按码元切分会把它们从中间截断，
 * 因此优先使用 Intl.Segmenter；运行环境不支持时退回按码点切分。
 *
 * @param text 原始文本
 * @returns 字素簇数组
 */
function toGraphemes(text: string): string[] {
  const Segmenter = (Intl as { Segmenter?: typeof Intl.Segmenter }).Segmenter;
  if (!Segmenter) return Array.from(text);
  const segmenter = new Segmenter(undefined, { granularity: "grapheme" });
  return Array.from(segmenter.segment(text), (item) => item.segment);
}

/**
 * 计算两行之间被改动的字符区间。
 *
 * 只剥离公共前缀与公共后缀，中间整体视为改动。这是最小且无参数的做法：
 * 不设相似度阈值，改一个字符就只高亮那一个字符。
 *
 * @param before 改动前文本
 * @param after 改动后文本
 * @returns 两侧各自的分段结果
 */
export function segmentLinePair(
  before: string,
  after: string
): { before: DiffSegment[]; after: DiffSegment[] } {
  const left = toGraphemes(before);
  const right = toGraphemes(after);

  // 1. 剥离公共前缀
  let head = 0;
  while (head < left.length && head < right.length && left[head] === right[head]) {
    head += 1;
  }
  // 2. 剥离公共后缀，注意不与前缀重叠
  let tail = 0;
  while (
    tail < left.length - head &&
    tail < right.length - head &&
    left[left.length - 1 - tail] === right[right.length - 1 - tail]
  ) {
    tail += 1;
  }

  return {
    before: buildSegments(left, head, tail),
    after: buildSegments(right, head, tail)
  };
}

/**
 * 按公共前后缀长度切出分段。
 *
 * @param graphemes 字素数组
 * @param head 公共前缀长度
 * @param tail 公共后缀长度
 * @returns 分段结果；无改动时返回单个未改动段
 */
function buildSegments(graphemes: string[], head: number, tail: number): DiffSegment[] {
  const middle = graphemes.slice(head, graphemes.length - tail).join("");
  const segments: DiffSegment[] = [];
  if (head > 0) segments.push({ text: graphemes.slice(0, head).join(""), changed: false });
  if (middle) segments.push({ text: middle, changed: true });
  if (tail > 0) {
    segments.push({ text: graphemes.slice(graphemes.length - tail).join(""), changed: false });
  }
  // 整行完全相同时也要返回内容，否则渲染出空行
  return segments.length > 0 ? segments : [{ text: graphemes.join(""), changed: false }];
}

/**
 * 为相邻的删除与新增行补齐字符级差异。
 *
 * 按位置配对：一段连续删除中的第 i 行与随后连续新增中的第 i 行成对。
 * 不做相似度筛选，配对完全由位置决定，行为可预测。
 *
 * @param lines 已解析的行序列
 * @returns 带字符级分段的行序列
 */
export function annotateInlineDiff(lines: DiffLine[]): DiffLine[] {
  const result = lines.map((line) => ({ ...line }));
  let index = 0;
  while (index < result.length) {
    if (result[index].kind !== "removed") {
      index += 1;
      continue;
    }
    // 1. 收集一段连续删除
    const removedStart = index;
    while (index < result.length && result[index].kind === "removed") index += 1;
    const removedEnd = index;
    // 2. 紧随其后的连续新增才构成配对
    const addedStart = index;
    while (index < result.length && result[index].kind === "added") index += 1;
    const addedEnd = index;

    const pairs = Math.min(removedEnd - removedStart, addedEnd - addedStart);
    for (let offset = 0; offset < pairs; offset += 1) {
      const removed = result[removedStart + offset];
      const added = result[addedStart + offset];
      const segments = segmentLinePair(removed.text, added.text);
      removed.segments = segments.before;
      added.segments = segments.after;
    }
  }
  return result;
}
