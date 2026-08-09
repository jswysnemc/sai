import { parseJsonRecord, stringField } from "../tool-renderers/tool-data";

/** 参与差异预览的工具最小形状 */
export type DiffSourceTool = {
  name: string;
  arguments?: string;
  argumentsPreview?: string;
  output?: string;
  status?: string;
};

/** 会产生文件差异的工具 */
const EDIT_TOOLS = ["edit_file", "write_file", "str_replace"];

/**
 * 从本轮编辑工具的参数与输出组装差异源文本。
 *
 * 优先取工具执行后返回的真实前后差异；只有历史运行没有保存它时，
 * 才回退到参数预览——参数里的大范围替换会被画成整文件改动，
 * 与实际改了什么并不相符。
 *
 * @param tools 本轮工具列表
 * @param path 目标文件路径；传 null 时汇总全部文件
 * @returns 可交给 DiffView 渲染的 patch 文本；无可用内容时返回空串
 */
export function buildTurnDiffSource(
  tools: readonly DiffSourceTool[],
  path: string | null
): string {
  const chunks: string[] = [];
  for (const tool of tools) {
    if (!EDIT_TOOLS.includes(tool.name)) continue;
    if (tool.status && tool.status !== "completed") continue;
    // 1. 真实差异可直接使用，它已经反映了工具实际改动的范围
    const output = parseJsonRecord(tool.output ?? "");
    const actualDiff = output ? stringField(output, "diff") : "";
    if (actualDiff && (!path || diffIncludesPath(actualDiff, path))) {
      chunks.push(actualDiff);
      continue;
    }
    // 2. 无真实差异时按工具类型从参数合成
    const synthesized = synthesizeFromArguments(tool, path);
    if (synthesized) chunks.push(synthesized);
  }
  return chunks.filter(Boolean).join("\n");
}

/**
 * 从工具参数合成差异文本。
 *
 * @param tool 目标工具
 * @param path 目标文件路径；传 null 时不做过滤
 * @returns patch 文本；参数不足时返回空串
 */
function synthesizeFromArguments(tool: DiffSourceTool, path: string | null): string {
  const args = parseJsonRecord(tool.arguments || tool.argumentsPreview || "");
  if (!args) return "";
  if (tool.name === "edit_file") {
    const patch = stringField(args, "patch");
    if (!patch) return "";
    if (path && !patchIncludesPath(patch, path)) return "";
    return path ? extractPatchForPath(patch, path) : patch;
  }
  const filePath = stringField(args, "path");
  if (!filePath || (path && !pathsMatch(filePath, path))) return "";
  const content = stringField(args, "content");
  const oldString = stringField(args, "old_string");
  const newString = stringField(args, "new_string");
  if (!content && !oldString && !newString) return "";
  return buildSyntheticPatch(
    filePath,
    oldString,
    newString || content,
    Boolean(content) && !oldString
  );
}

/**
 * 判断 unified diff 是否属于指定文件。
 *
 * @param diff unified diff 文本
 * @param path 目标文件路径
 * @returns 是否匹配
 */
function diffIncludesPath(diff: string, path: string): boolean {
  return diff.split("\n").some((line) => {
    if (!line.startsWith("+++ ") && !line.startsWith("--- ")) return false;
    const candidate = line.slice(4).trim();
    if (candidate === "/dev/null") return false;
    return pathsMatch(candidate.replace(/^[ab]\//, ""), path);
  });
}

/**
 * 判断两个路径是否指向同一文件。
 *
 * changed_files 常返回绝对路径，而工具参数多为工作区相对路径，
 * 因此在归一化分隔符后按后缀对齐。
 *
 * @param left 路径一
 * @param right 路径二
 * @returns 是否匹配
 */
function pathsMatch(left: string, right: string): boolean {
  const a = normalizePath(left);
  const b = normalizePath(right);
  if (a === b) return true;
  return a.endsWith(`/${b}`) || b.endsWith(`/${a}`);
}

/**
 * 归一化路径分隔符并去掉开头的 ./ 前缀。
 *
 * @param path 原始路径
 * @returns 归一化路径
 */
function normalizePath(path: string): string {
  return path.replaceAll("\\", "/").replace(/^\.\//, "");
}

/**
 * 判断 patch 是否包含指定路径。
 *
 * @param patch Codex patch
 * @param path 文件路径
 * @returns 是否包含
 */
function patchIncludesPath(patch: string, path: string): boolean {
  return patch.split("\n").some((line) =>
    line.startsWith("*** Add File: ")
    || line.startsWith("*** Delete File: ")
    || line.startsWith("*** Update File: ")
      ? pathsMatch(line.slice(line.indexOf(": ") + 2).trim(), path)
      : false
  );
}

/**
 * 从多文件 patch 中提取单个路径的片段。
 *
 * @param patch 完整 patch
 * @param path 目标路径
 * @returns 单文件 patch；未命中时返回空串
 */
function extractPatchForPath(patch: string, path: string): string {
  const result: string[] = ["*** Begin Patch"];
  let capturing = false;
  for (const line of patch.split("\n")) {
    if (/^\*\*\* (Add|Delete|Update) File: /.test(line)) {
      const filePath = line.slice(line.indexOf(": ") + 2).trim();
      capturing = pathsMatch(filePath, path);
      if (capturing) result.push(line);
      continue;
    }
    if (line.startsWith("*** End Patch")) {
      capturing = false;
      continue;
    }
    if (capturing) result.push(line);
  }
  result.push("*** End Patch");
  return result.length > 2 ? result.join("\n") : "";
}

/**
 * 将 write_file / str_replace 参数转成可预览的简易 patch。
 *
 * @param path 文件路径
 * @param oldText 旧文本
 * @param newText 新文本
 * @param isAdd 是否整文件新增或覆盖
 * @returns Codex 风格 patch 文本
 */
function buildSyntheticPatch(
  path: string,
  oldText: string,
  newText: string,
  isAdd: boolean
): string {
  if (isAdd) {
    const body = newText.split("\n").map((line) => `+${line}`).join("\n");
    return `*** Begin Patch\n*** Add File: ${path}\n${body}\n*** End Patch`;
  }
  const removed = oldText ? oldText.split("\n").map((line) => `-${line}`).join("\n") : "";
  const added = newText.split("\n").map((line) => `+${line}`).join("\n");
  return `*** Begin Patch\n*** Update File: ${path}\n@@\n${removed}${removed && added ? "\n" : ""}${added}\n*** End Patch`;
}
