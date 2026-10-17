/** 请求在内置浏览器中打开 HTML 文件。 */
export const OPEN_HTML_IN_BROWSER_EVENT = "sai:open-html-in-browser";

/**
 * 判断路径指向 HTML 文件。
 *
 * @param path 工作区相对路径或绝对路径
 * @returns 扩展名是 html 或 htm 时为 true
 */
export function isHtmlFile(path: string): boolean {
  const name = path.split(/[/\\]/).pop()?.trim().toLowerCase() ?? "";
  return name.endsWith(".html") || name.endsWith(".htm");
}

/**
 * 通知工作区打开这个 HTML 文件。
 *
 * @param path 工作区相对路径或绝对路径
 * @returns 无返回值
 */
export function requestOpenHtmlInBrowser(path: string): void {
  window.dispatchEvent(new CustomEvent(OPEN_HTML_IN_BROWSER_EVENT, { detail: path }));
}
