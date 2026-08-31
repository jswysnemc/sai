/**
 * 判断当前环境是否按 Apple 修饰键习惯展示快捷键。
 *
 * @returns 应显示 ⌘ 时为 true
 */
export function isApplePlatform(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Mac|iPhone|iPad|iPod/iu.test(`${navigator.platform} ${navigator.userAgent}`);
}

/**
 * 返回当前平台的主键修饰符标签。
 *
 * @returns macOS 为 ⌘，其余为 Ctrl
 */
export function modKeyLabel(): string {
  return isApplePlatform() ? "⌘" : "Ctrl";
}
