export type RenameCommand = {
  title: string;
};

/**
 * 解析输入区 `/rename` 命令。
 * 兼容全角斜杠、中文别名 `/重命名`，以及标题紧贴命令名的写法。
 *
 * @param value 当前输入文本
 * @returns 命令匹配时返回标题，否则返回 null
 */
export function parseRenameCommand(value: string): RenameCommand | null {
  let text = value
    .replace(/^\uFEFF/u, "")
    .replace(/[\u200B-\u200D\uFEFF]/gu, "")
    .trim();
  if (!text) return null;
  if (text.startsWith("／")) {
    text = `/${text.slice(1)}`;
  }
  const withSpace = text.match(/^\/(?:rename|重命名)(?:[\s\u00a0\u3000]+([\s\S]*))?$/iu);
  if (withSpace) {
    return { title: (withSpace[1] ?? "").trim() };
  }
  const gluedNonAscii = text.match(/^\/(?:rename|重命名)([^\x00-\x7F][\s\S]*)$/iu);
  if (gluedNonAscii) {
    return { title: gluedNonAscii[1].trim() };
  }
  return null;
}
