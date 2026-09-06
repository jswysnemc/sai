import type { DiffFile } from "../../chat/tool-renderers/diff/diff-model";

/** 超过此行数的文件按需展开，避免初次审阅生成大量代码节点。 */
const LARGE_DIFF_LINES = 300;

/**
 * 计算文件折叠状态；用户操作优先，选中文件始终可以直接进入正文。
 * @param file 文件差异
 * @param overrides 用户明确设置的折叠状态
 * @param selectedPath 从变更列表选中的文件
 * @returns 是否折叠当前文件
 */
export function isDiffFileCollapsed(file: DiffFile, overrides: ReadonlyMap<string, boolean>, selectedPath: string | null): boolean {
  return overrides.get(file.path) ?? (file.lines.length > LARGE_DIFF_LINES && file.path !== selectedPath);
}
