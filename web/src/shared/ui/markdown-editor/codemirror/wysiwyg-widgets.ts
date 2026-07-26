import { EditorView, WidgetType } from "@codemirror/view";

/**
 * 内联图片部件。
 *
 * 仅对可直接加载的绝对地址生效：工作区里的相对路径没有可靠的基准目录，
 * 渲染出来只会是碎图，那种情况保留源码文本更有用。
 */
export class ImageWidget extends WidgetType {
  constructor(
    private readonly url: string,
    private readonly alt: string
  ) {
    super();
  }

  /**
   * 判断两个部件是否等价，避免重复重建 DOM。
   *
   * @param other 另一个部件
   * @returns 地址与替代文本都相同时为 true
   */
  eq(other: ImageWidget) {
    return other.url === this.url && other.alt === this.alt;
  }

  /**
   * 构建图片元素。
   *
   * @returns 承载图片的行内块元素
   */
  toDOM() {
    const wrap = document.createElement("span");
    wrap.className = "cm-md-image";
    const image = document.createElement("img");
    image.src = this.url;
    image.alt = this.alt;
    image.loading = "lazy";
    wrap.appendChild(image);
    return wrap;
  }
}

/**
 * 判断图片地址是否可以直接渲染。
 *
 * @param url 图片地址
 * @returns 绝对地址或内联数据时为 true
 */
export function isRenderableImageUrl(url: string): boolean {
  return /^(https?:\/\/|data:image\/|\/)/i.test(url);
}

/** 水平分隔线部件。 */
export class RuleWidget extends WidgetType {
  /**
   * 判断两个部件是否等价。
   *
   * @param other 另一个部件
   * @returns 恒为 true，分隔线没有可变状态
   */
  eq(other: RuleWidget) {
    return other instanceof RuleWidget;
  }

  /**
   * 构建分隔线元素。
   *
   * @returns 分隔线容器
   */
  toDOM() {
    const wrap = document.createElement("span");
    wrap.className = "cm-md-rule";
    wrap.appendChild(document.createElement("hr"));
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
   * @returns 勾选框元素
   */
  toDOM(view: EditorView) {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.className = "cm-md-task";
    box.checked = this.checked;
    box.addEventListener("mousedown", (event) => {
      // 1. 阻止默认行为，避免点击时编辑器抢走焦点并移动光标
      event.preventDefault();
      // 2. 直接改写标记字符，源码与视图保持单一数据源
      view.dispatch({
        changes: { from: this.from, to: this.to, insert: this.checked ? "[ ]" : "[x]" },
      });
    });
    return box;
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
