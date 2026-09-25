import { EditorSelection, EditorState, type Transaction, type TransactionSpec } from "@codemirror/state";
import { Decoration, EditorView } from "@codemirror/view";
import { wysiwygBlockDecorations, type GuardRange } from "./wysiwyg-block-decorations";
import { wysiwygDecorations } from "./wysiwyg-decorations";
import { analyzeLinePrefix } from "./wysiwyg-line-prefix";

/** 由部件内部发起、允许改写受保护区域的事件类型。 */
const TRUSTED_EVENTS = ["input.table", "input.toggle-task", "input.structure"];

/**
 * 所见即所得模式的光标与结构保护。
 *
 * 1. 删除保护：键盘删除、粘贴等用户输入若会破坏隐藏的围栏行或表格，整笔取消
 * 2. 光标归位：光标落进看不见的位置（行首前缀、隐藏围栏行、表格源码）时，
 *    按移动方向挪到最近的可见位置
 * 3. 原子区间：行首前缀连同上一行换行符一起作为原子区间，左右移动一步跨过
 */
export const wysiwygGuard = [
  EditorState.transactionFilter.of((transaction) => guardTransaction(transaction)),
  EditorView.atomicRanges.of((view) => view.plugin(wysiwygDecorations)?.atomic ?? Decoration.none),
];

/**
 * 处理一笔事务。
 *
 * @param transaction 待提交的事务
 * @returns 原事务、修正后的事务或空数组（取消）
 */
function guardTransaction(transaction: Transaction): Transaction | TransactionSpec | readonly TransactionSpec[] {
  const guards = transaction.startState.field(wysiwygBlockDecorations, false)?.guards ?? [];
  if (transaction.docChanged) {
    if (TRUSTED_EVENTS.some((event) => transaction.isUserEvent(event))) return transaction;
    if (isUserEdit(transaction) && breaksGuard(transaction, guards)) return [];
    return transaction;
  }
  if (!transaction.selection) return transaction;
  const selection = normalizeSelection(transaction, guards);
  return selection ? [transaction, { selection, sequential: true }] : transaction;
}

/**
 * 判断事务是否来自用户的直接编辑。
 *
 * @param transaction 事务
 * @returns 输入、删除、移动或撤销类事件时为 true
 */
function isUserEdit(transaction: Transaction): boolean {
  return ["input", "delete", "move"].some((event) => transaction.isUserEvent(event));
}

/**
 * 判断事务是否破坏了某个受保护区域。
 *
 * 区域被整体删除（如全选后删除）视为合法；只动了一部分，
 * 或删掉了它与相邻行之间的换行、使其不再独占整行，都视为破坏。
 *
 * @param transaction 事务
 * @param guards 事务开始前的受保护区域
 * @returns 有区域被破坏时为 true
 */
function breaksGuard(transaction: Transaction, guards: readonly GuardRange[]): boolean {
  const before = transaction.startState.doc;
  const after = transaction.newDoc;
  return guards.some((guard) => {
    const touched = transaction.changes.touchesRange(Math.max(0, guard.from - 1), Math.min(before.length, guard.to + 1));
    if (!touched || transaction.changes.touchesRange(guard.from, guard.to) === "cover") return false;
    const from = transaction.changes.mapPos(guard.from, 1);
    const to = transaction.changes.mapPos(guard.to, -1);
    if (after.sliceString(from, to) !== before.sliceString(guard.from, guard.to)) return true;
    return after.lineAt(from).from !== from || after.lineAt(to).to !== to;
  });
}

/**
 * 把落在不可见位置的光标挪到最近的可见位置。
 *
 * @param transaction 只改选区的事务
 * @param guards 受保护区域
 * @returns 修正后的选区；无需修正时为 null
 */
function normalizeSelection(transaction: Transaction, guards: readonly GuardRange[]): EditorSelection | null {
  const state = transaction.startState;
  const selection = transaction.newSelection;
  const previousHead = state.selection.main.head;
  let changed = false;
  const ranges = selection.ranges.map((range) => {
    const forward = range.head >= previousHead;
    const head = visibleHead(state, range.head, forward, guards);
    if (head === range.head) return range;
    changed = true;
    return range.empty ? EditorSelection.cursor(head) : EditorSelection.range(range.anchor, head);
  });
  return changed ? EditorSelection.create(ranges, selection.mainIndex) : null;
}

/**
 * 计算某个位置对应的可见位置。
 *
 * @param state 编辑器状态
 * @param head 光标位置
 * @param forward 是否向前（向右、向下）移动
 * @param guards 受保护区域
 * @returns 可见位置
 */
function visibleHead(state: EditorState, head: number, forward: boolean, guards: readonly GuardRange[]): number {
  // 1. 隐藏的围栏行与表格源码内部
  for (const guard of guards) {
    const inside = guard.kind === "table" ? head > guard.from && head < guard.to : head >= guard.from && head <= guard.to;
    if (inside) return forward ? guard.after : guard.before;
  }
  // 2. 行首隐藏前缀：一律挪到正文起点
  const line = state.doc.lineAt(head);
  const prefix = analyzeLinePrefix(state, line);
  if (prefix && head >= prefix.from && head < prefix.to) return prefix.to;
  return head;
}
