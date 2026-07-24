export type ComposerSelection = {
  start: number;
  end: number;
};

export type ComposerEditSnapshot = {
  value: string;
  selection: ComposerSelection;
};

const MAX_HISTORY = 200;

/**
 * 聊天输入区的撤销/重做栈。
 *
 * contentEditable 在受控重建 DOM 后会丢失浏览器原生历史，因此用应用层栈承接 Ctrl+Z / Ctrl+Y。
 */
export class ComposerEditHistory {
  private undoStack: ComposerEditSnapshot[] = [];
  private redoStack: ComposerEditSnapshot[] = [];
  private lastCommitted: ComposerEditSnapshot | null = null;

  /**
   * 用当前内容初始化基线，清空撤销/重做。
   *
   * @param snapshot 当前文本与选区
   * @returns 无返回值
   */
  reset(snapshot: ComposerEditSnapshot): void {
    this.undoStack = [];
    this.redoStack = [];
    this.lastCommitted = cloneSnapshot(snapshot);
  }

  /**
   * 在用户编辑生效后记录一步历史。
   *
   * @param previous 变更前快照
   * @param next 变更后快照
   * @returns 无返回值
   */
  record(previous: ComposerEditSnapshot, next: ComposerEditSnapshot): void {
    if (previous.value === next.value && sameSelection(previous.selection, next.selection)) {
      this.lastCommitted = cloneSnapshot(next);
      return;
    }
    // 1. 连续相同文本不重复入栈
    if (this.lastCommitted && this.lastCommitted.value === previous.value) {
      this.undoStack.push(cloneSnapshot(this.lastCommitted));
    } else {
      this.undoStack.push(cloneSnapshot(previous));
    }
    if (this.undoStack.length > MAX_HISTORY) {
      this.undoStack.splice(0, this.undoStack.length - MAX_HISTORY);
    }
    this.redoStack = [];
    this.lastCommitted = cloneSnapshot(next);
  }

  /**
   * 弹出撤销目标。
   *
   * @param current 撤销前的当前快照
   * @returns 目标快照；无可撤销时 null
   */
  undo(current: ComposerEditSnapshot): ComposerEditSnapshot | null {
    const target = this.undoStack.pop();
    if (!target) return null;
    this.redoStack.push(cloneSnapshot(current));
    this.lastCommitted = cloneSnapshot(target);
    return target;
  }

  /**
   * 弹出重做目标。
   *
   * @param current 重做前的当前快照
   * @returns 目标快照；无可重做时 null
   */
  redo(current: ComposerEditSnapshot): ComposerEditSnapshot | null {
    const target = this.redoStack.pop();
    if (!target) return null;
    this.undoStack.push(cloneSnapshot(current));
    this.lastCommitted = cloneSnapshot(target);
    return target;
  }

  /**
   * 是否存在可撤销项。
   *
   * @returns 可撤销时 true
   */
  canUndo(): boolean {
    return this.undoStack.length > 0;
  }

  /**
   * 是否存在可重做项。
   *
   * @returns 可重做时 true
   */
  canRedo(): boolean {
    return this.redoStack.length > 0;
  }
}

type KeyModEvent = {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
};

/**
 * 判断是否为撤销快捷键（Ctrl/Cmd+Z，不含 Shift）。
 *
 * @param event 键盘事件
 * @returns 匹配时 true
 */
export function isUndoShortcut(event: KeyModEvent): boolean {
  if (event.altKey || event.shiftKey) return false;
  if (!(event.ctrlKey || event.metaKey)) return false;
  return event.key === "z" || event.key === "Z";
}

/**
 * 判断是否为重做快捷键（Ctrl/Cmd+Y，或 Ctrl/Cmd+Shift+Z）。
 *
 * @param event 键盘事件
 * @returns 匹配时 true
 */
export function isRedoShortcut(event: KeyModEvent): boolean {
  if (event.altKey) return false;
  if (!(event.ctrlKey || event.metaKey)) return false;
  if (event.key === "y" || event.key === "Y") return !event.shiftKey;
  return (event.key === "z" || event.key === "Z") && event.shiftKey;
}

/**
 * 复制快照，避免栈内引用被后续编辑改写。
 *
 * @param snapshot 原始快照
 * @returns 深拷贝快照
 */
function cloneSnapshot(snapshot: ComposerEditSnapshot): ComposerEditSnapshot {
  return {
    value: snapshot.value,
    selection: { start: snapshot.selection.start, end: snapshot.selection.end }
  };
}

/**
 * 比较两个选区是否一致。
 *
 * @param left 左选区
 * @param right 右选区
 * @returns 一致时 true
 */
function sameSelection(left: ComposerSelection, right: ComposerSelection): boolean {
  return left.start === right.start && left.end === right.end;
}
