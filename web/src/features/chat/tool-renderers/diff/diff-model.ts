/** diff 行的类型 */
export type DiffLineKind = "added" | "removed" | "context" | "hunk" | "no-newline";

/** 文件级变更状态 */
export type DiffFileStatus =
  | "added"
  | "deleted"
  | "renamed"
  | "modified"
  | "binary"
  | "mode-changed";

/** 单行差异中被改动的字符区间 */
export type DiffSegment = {
  text: string;
  changed: boolean;
};

export type DiffLine = {
  kind: DiffLineKind;
  text: string;
  oldLine?: number;
  newLine?: number;
  /** 与相邻行配对后得出的字符级差异；未配对时为空 */
  segments?: DiffSegment[];
};

export type DiffFile = {
  path: string;
  /** 重命名前的路径 */
  oldPath?: string;
  status: DiffFileStatus;
  added: number;
  removed: number;
  lines: DiffLine[];
};

/**
 * 返回文件状态的双语展示名。
 *
 * @param status 文件变更状态
 * @returns 英文与中文名称
 */
export function diffStatusLabel(status: DiffFileStatus): { en: string; zh: string } {
  switch (status) {
    case "added":
      return { en: "Added", zh: "新增" };
    case "deleted":
      return { en: "Deleted", zh: "删除" };
    case "renamed":
      return { en: "Renamed", zh: "重命名" };
    case "binary":
      return { en: "Binary", zh: "二进制" };
    case "mode-changed":
      return { en: "Mode changed", zh: "权限变更" };
    case "modified":
      return { en: "Modified", zh: "修改" };
  }
}
