type TerminalDisposalTarget = {
  element?: Pick<HTMLElement, "remove">;
  dispose: () => void;
};

/**
 * 【Web 终端】【实例释放】立即移除视图，等待初始化回调完成后释放资源。
 * @param terminal 当前组件持有的终端实例
 * @returns 无返回值
 */
export function disposeTerminalView(terminal: TerminalDisposalTarget): void {
  terminal.element?.remove();
  // 1. Xterm 5.5 的视口初始化使用零延迟任务，销毁必须排在该任务之后
  setTimeout(() => terminal.dispose(), 0);
}
