const STORAGE_KEY = "sai.file-tree.show-hidden";

/**
 * 读取文件树的隐藏条目显示偏好，未设置时默认隐藏。
 * @returns 是否显示以点开头的文件和目录
 */
export function readShowHiddenFiles(): boolean {
  try {
    return globalThis.localStorage?.getItem(STORAGE_KEY) === "on";
  } catch {
    return false;
  }
}

/**
 * 保存隐藏条目显示偏好，供页面刷新或切换工作区后使用。
 * @param showHidden 是否显示隐藏条目
 * @returns 无返回值
 */
export function writeShowHiddenFiles(showHidden: boolean): void {
  try {
    globalThis.localStorage?.setItem(STORAGE_KEY, showHidden ? "on" : "off");
  } catch {
    // 1. 存储不可用时保留当前页面状态
  }
}
