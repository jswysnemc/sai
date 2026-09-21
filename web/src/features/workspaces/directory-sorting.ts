import type { DirectoryEntry } from "../../api/contracts";

/**
 * 按日期命名优先、隐藏目录靠后和自然名称顺序排列目录。
 *
 * @param entries 服务端目录条目
 * @returns 排序后的新数组，不修改输入
 */
export function sortDirectoryEntries(entries: DirectoryEntry[]): DirectoryEntry[] {
  return [...entries].sort((left, right) => {
    const leftDate = dateFromDirectoryName(left.name);
    const rightDate = dateFromDirectoryName(right.name);
    if (leftDate !== null || rightDate !== null) {
      if (leftDate === null) return 1;
      if (rightDate === null) return -1;
      if (leftDate !== rightDate) return rightDate - leftDate;
    }
    const leftHidden = left.name.startsWith(".") ? 1 : 0;
    const rightHidden = right.name.startsWith(".") ? 1 : 0;
    if (leftHidden !== rightHidden) return leftHidden - rightHidden;
    return left.name.localeCompare(right.name, undefined, { numeric: true, sensitivity: "base" });
  });
}

/**
 * 解析常见的 YYYY-MM-DD、YYYY_MM_DD 和 YYYYMMDD 目录名。
 *
 * @param name 目录名称
 * @returns UTC 时间戳；名称不含有效日期时返回 null
 */
export function dateFromDirectoryName(name: string): number | null {
  const match = /(^|[^\d])(\d{4})[-_.]?(\d{2})(?:[-_.]?(\d{2}))?(?=$|[^\d])/u.exec(name);
  if (!match) return null;
  const year = Number(match[2]);
  const month = Number(match[3]);
  const day = Number(match[4] ?? "1");
  const date = new Date(Date.UTC(year, month - 1, day));
  if (
    month < 1 ||
    month > 12 ||
    day < 1 ||
    date.getUTCFullYear() !== year ||
    date.getUTCMonth() !== month - 1 ||
    date.getUTCDate() !== day
  ) {
    return null;
  }
  return date.getTime();
}
