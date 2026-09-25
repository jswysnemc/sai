import { EditorView, WidgetType } from "@codemirror/view";

/** 各级无序列表的符号，超出层数时循环使用。 */
const BULLET_CLASSES = ["is-disc", "is-circle", "is-square"];

/**
 * 无序列表圆点部件。
 *
 * 替换行首缩进与 `-`/`*`/`+` 符号，宽度固定，
 * 与行样式里的悬挂缩进配合，折行后的文字与首行文字对齐。
 */
export class BulletWidget extends WidgetType {
  constructor(private readonly depth: number) {
    super();
  }

  /**
   * 判断两个部件是否等价。
   *
   * @param other 另一个部件
   * @returns 层级相同时为 true
   */
  eq(other: BulletWidget) {
    return other.depth === this.depth;
  }

  /**
   * 构建圆点元素。
   *
   * @returns 固定宽度的圆点容器
   */
  toDOM() {
    const wrap = document.createElement("span");
    wrap.className = `cm-md-list-marker cm-md-bullet ${BULLET_CLASSES[(this.depth - 1) % BULLET_CLASSES.length]}`;
    wrap.setAttribute("aria-hidden", "true");
    return wrap;
  }
}

/** 有序列表序号部件：保留原文序号，只统一宽度与颜色。 */
export class OrderedWidget extends WidgetType {
  constructor(private readonly label: string) {
    super();
  }

  /**
   * 判断两个部件是否等价。
   *
   * @param other 另一个部件
   * @returns 序号相同时为 true
   */
  eq(other: OrderedWidget) {
    return other.label === this.label;
  }

  /**
   * 构建序号元素。
   *
   * @returns 固定宽度的序号容器
   */
  toDOM() {
    const wrap = document.createElement("span");
    wrap.className = "cm-md-list-marker cm-md-ordered";
    wrap.textContent = this.label;
    wrap.setAttribute("aria-hidden", "true");
    return wrap;
  }
}

/** 任务列表勾选框部件，点击直接改写源码中的标记。 */
export class TaskWidget extends WidgetType {
  constructor(
    private readonly checked: boolean,
    private readonly from: number,
    private readonly to: number
  ) {
    super();
  }

  /**
   * 判断两个部件是否等价。
   *
   * @param other 另一个部件
   * @returns 勾选状态与位置都相同时为 true
   */
  eq(other: TaskWidget) {
    return other.checked === this.checked && other.from === this.from && other.to === this.to;
  }

  /**
   * 构建勾选框并绑定切换行为。
   *
   * @param view 所属编辑器视图
   * @returns 固定宽度的勾选框容器
   */
  toDOM(view: EditorView) {
    const wrap = document.createElement("span");
    wrap.className = "cm-md-list-marker cm-md-task-marker";
    const box = document.createElement("input");
    box.type = "checkbox";
    box.className = "cm-md-task";
    box.checked = this.checked;
    box.tabIndex = -1;
    box.addEventListener("mousedown", (event) => {
      // 1. 阻止默认行为，避免点击时编辑器抢走焦点并移动光标
      event.preventDefault();
      if (!view.state.facet(EditorView.editable)) return;
      // 2. 直接改写标记字符，源码与视图保持单一数据源
      view.dispatch({
        changes: { from: this.from, to: this.to, insert: this.checked ? "[ ]" : "[x]" },
        userEvent: "input.toggle-task",
      });
    });
    wrap.appendChild(box);
    return wrap;
  }

  /**
   * 声明该部件自行处理事件。
   *
   * @returns 恒为 true，交互由部件内部完成
   */
  ignoreEvent() {
    return true;
  }
}
