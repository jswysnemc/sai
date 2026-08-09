/**
 * 目录浏览的「输入框即状态源」解析。
 *
 * 路径输入框同时承担导航与过滤：以 `/` 结尾的部分是当前浏览目录，
 * 最后一段是子目录名过滤词。输入 `/home/sn` 即浏览 `/home/` 并
 * 过滤出 `sn` 开头的目录，无需独立搜索框。
 */

/**
 * 归一路径分隔符为正斜杠。
 *
 * 参数:
 * - `value`: 原始输入
 *
 * 返回:
 * - 反斜杠替换为正斜杠的文本（保留尾斜杠）
 */
export function normalizeSlashes(value: string): string {
  return value.replace(/\\/g, "/");
}

/**
 * 提取输入中的浏览目录部分。
 *
 * 参数:
 * - `value`: 路径输入框文本
 *
 * 返回:
 * - 以 `/` 结尾的目录前缀；无分隔符时返回空串（表示不跳转）
 */
export function directoryOfInput(value: string): string {
  const normalized = normalizeSlashes(value);
  if (normalized.endsWith("/")) return normalized;
  const lastSlash = normalized.lastIndexOf("/");
  if (lastSlash < 0) return "";
  return normalized.slice(0, lastSlash + 1);
}

/**
 * 提取输入中的过滤词部分。
 *
 * 参数:
 * - `value`: 路径输入框文本
 *
 * 返回:
 * - 最后一个 `/` 之后的文本；目录形态输入返回空串
 */
export function filterOfInput(value: string): string {
  const normalized = normalizeSlashes(value);
  if (normalized.endsWith("/")) return "";
  const lastSlash = normalized.lastIndexOf("/");
  return normalized.slice(lastSlash + 1);
}

/**
 * 保证路径以斜杠结尾，供输入框展示目录形态。
 *
 * 参数:
 * - `path`: 目录绝对路径
 *
 * 返回:
 * - 以 `/` 结尾的路径
 */
export function ensureTrailingSlash(path: string): string {
  const normalized = normalizeSlashes(path);
  return normalized.endsWith("/") ? normalized : `${normalized}/`;
}

/**
 * 去掉尾斜杠得到可提交路径；保留文件系统根。
 *
 * 参数:
 * - `path`: 目录形态路径
 *
 * 返回:
 * - 无尾斜杠的路径；`/` 与 `C:/` 这类根原样返回
 */
export function stripTrailingSlash(path: string): string {
  const normalized = normalizeSlashes(path);
  if (normalized === "/" || /^[A-Za-z]:\/$/.test(normalized)) return normalized;
  return normalized.endsWith("/") ? normalized.slice(0, -1) : normalized;
}

/**
 * 求目录形态路径的最后一段目录名。
 *
 * 参数:
 * - `path`: 以 `/` 结尾的目录路径
 *
 * 返回:
 * - 最后一段目录名；根路径返回空串
 */
export function lastSegmentOf(path: string): string {
  const stripped = stripTrailingSlash(path);
  const lastSlash = stripped.lastIndexOf("/");
  return lastSlash < 0 ? stripped : stripped.slice(lastSlash + 1);
}
