/**
 * 把 Markdown 文件里的相对图片地址解析为工作区图片接口地址。
 *
 * 绝对网址、内联数据与站内绝对路径原样放行；
 * 相对路径以 Markdown 文件所在目录为基准，支持 `./` 与 `../`。
 *
 * @param filePath Markdown 文件在工作区内的路径
 * @param src 文档中的图片地址
 * @returns 可加载的地址；越出工作区根目录时为 null
 */
export function resolveWorkspaceImage(filePath: string, src: string): string | null {
  if (/^(https?:\/\/|data:image\/|\/)/i.test(src)) return src;
  if (/^[a-z][a-z0-9+.-]*:/i.test(src)) return null;
  const parts = filePath.split("/").slice(0, -1);
  let decoded = src;
  try {
    decoded = decodeURI(src);
  } catch {
    // 编码不合法时按原文处理
  }
  for (const segment of decoded.split(/[?#]/)[0].split("/")) {
    if (!segment || segment === ".") continue;
    if (segment === "..") {
      if (!parts.length) return null;
      parts.pop();
    } else {
      parts.push(segment);
    }
  }
  return `/api/workspace/image?path=${encodeURIComponent(parts.join("/"))}`;
}
