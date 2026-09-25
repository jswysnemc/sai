import { normalizeSlashes } from "./directory-path-input";

export type DroppedDirectory = {
  /** 浏览器交出的绝对路径；没有时为 null */
  path: string | null;
  /** 目录名，供没有绝对路径时到允许根下查找 */
  name: string | null;
};

/**
 * 从拖放数据里解析目录。
 *
 * 文件管理器通常给出 `file://` URI。浏览器出于安全限制只给文件名时，
 * 只返回目录名，由调用方到已知根目录下确认。
 *
 * @param uriList `text/uri-list`
 * @param plain `text/plain`
 * @param fileName 拖入项的文件名
 * @returns 绝对路径和目录名；都没有时两者为 null
 */
export function parseDroppedDirectory(uriList: string, plain: string, fileName: string): DroppedDirectory {
  const uri = uriList.split(/\r?\n/u).map((line) => line.trim()).find((line) => line && !line.startsWith("#"));
  const fromUri = uri ? pathFromFileUri(uri) : null;
  if (fromUri) return { path: fromUri, name: lastSegment(fromUri) };
  const text = plain.trim();
  const fromPlainUri = text.startsWith("file://") ? pathFromFileUri(text) : null;
  if (fromPlainUri) return { path: fromPlainUri, name: lastSegment(fromPlainUri) };
  if (text.startsWith("/") || /^[A-Za-z]:[\\/]/u.test(text)) {
    const path = normalizeDroppedPath(text);
    return { path, name: lastSegment(path) };
  }
  const name = fileName.trim();
  return { path: null, name: name || null };
}

/**
 * 把 `file://` URI 转成目录路径。
 *
 * @param uri 文件 URI
 * @returns 路径；不是 file URI 时返回 null
 */
function pathFromFileUri(uri: string): string | null {
  const trimmed = uri.trim();
  if (!trimmed.startsWith("file://")) return null;
  try {
    return normalizeDroppedPath(decodeURIComponent(new URL(trimmed).pathname));
  } catch {
    return normalizeDroppedPath(trimmed.slice("file://".length));
  }
}

/**
 * 兼容 Windows 文件 URI 以斜杠包裹盘符的格式。
 *
 * @param path 拖放数据中的原始路径
 * @returns 可提交给服务端目录接口的标准路径
 */
export function normalizeDroppedPath(path: string): string {
  const normalized = normalizeSlashes(path.trim());
  return normalized.replace(/^\/(?:([A-Za-z]):\/)/u, "$1:/");
}

/** 取路径最后一段作为目录名。 */
function lastSegment(path: string): string | null {
  const segment = path.split("/").filter(Boolean).at(-1);
  return segment || null;
}
