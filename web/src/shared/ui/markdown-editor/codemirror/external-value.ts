/**
 * 外部 value 与编辑器文档的同步判定。
 *
 * 受控组件的经典竞态：编辑器连续派发两笔修改后，父组件可能带着
 * 中间版本的 value 重新渲染，若直接按"值不同就回写"处理，
 * 后一笔修改会被旧值覆盖（实测表现为输入回退、光标跳动）。
 * 解决办法是记录编辑器自己发出过的内容队列，凡是队列里出现过的
 * value 都视为回声（echo），不再写回编辑器。
 */

/** 同步动作分类。 */
export type ExternalValueAction =
  | { kind: "idle" }
  | { kind: "echo" }
  | { kind: "replace" };

/** 回声队列的容量上限，防止长会话下无限增长。 */
export const ECHO_QUEUE_LIMIT = 32;

/**
 * 记录编辑器发出的一笔内容。
 *
 * @param queue 回声队列，就地修改
 * @param value 编辑器刚发出的文档内容
 * @returns 无
 */
export function recordEmittedValue(queue: string[], value: string): void {
  queue.push(value);
  if (queue.length > ECHO_QUEUE_LIMIT) {
    queue.splice(0, queue.length - ECHO_QUEUE_LIMIT);
  }
}

/**
 * 判定一笔外部 value 应如何同步进编辑器。
 *
 * 命中回声时会把队列消费到该项为止（保留其后仍在途的回声）；
 * 判定为真正的外部变化时清空队列。
 *
 * @param value 父组件传入的最新内容
 * @param currentDoc 编辑器当前文档内容
 * @param queue 回声队列，就地修改
 * @returns 同步动作：idle 不需处理、echo 忽略、replace 整体替换
 */
export function resolveExternalValue(
  value: string,
  currentDoc: string,
  queue: string[]
): ExternalValueAction {
  if (value === currentDoc) {
    queue.length = 0;
    return { kind: "idle" };
  }
  const echoIndex = queue.indexOf(value);
  if (echoIndex >= 0) {
    queue.splice(0, echoIndex + 1);
    return { kind: "echo" };
  }
  queue.length = 0;
  return { kind: "replace" };
}
