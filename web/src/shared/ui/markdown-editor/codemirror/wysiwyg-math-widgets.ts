import { EditorView, WidgetType } from "@codemirror/view";

type Katex = typeof import("katex").default;

/** KaTeX 模块，首次用到公式时才异步加载。 */
let katexModule: Katex | null = null;
let katexLoading: Promise<Katex> | null = null;

/**
 * 按需加载 KaTeX。
 *
 * @returns 加载完成的 KaTeX 模块
 */
function loadKatex(): Promise<Katex> {
  katexLoading ??= import("katex").then((module) => {
    katexModule = module.default;
    return katexModule;
  });
  return katexLoading;
}

/**
 * 把公式渲染进元素；KaTeX 尚未加载时先显示源码，加载完再替换。
 *
 * @param element 目标元素
 * @param tex 公式源码，不含定界符
 * @param display 是否为块级公式
 * @param view 所属编辑器视图，渲染后通知重新测量高度
 * @returns 无
 */
function renderTex(element: HTMLElement, tex: string, display: boolean, view: EditorView): void {
  const paint = (katex: Katex) => {
    try {
      katex.render(tex, element, { displayMode: display, throwOnError: false, strict: "ignore" });
      element.classList.remove("is-pending");
    } catch {
      element.textContent = tex;
      element.classList.add("is-error");
    }
  };
  if (katexModule) {
    paint(katexModule);
    return;
  }
  element.textContent = tex;
  element.classList.add("is-pending");
  void loadKatex().then((katex) => {
    if (!element.isConnected) return;
    paint(katex);
    view.requestMeasure();
  });
}

/**
 * 点击渲染结果时把光标送回源码，由装饰层切换为可编辑的源码。
 *
 * @param element 部件根元素
 * @param view 所属编辑器视图
 * @param anchor 光标落点
 * @returns 无
 */
function bindReveal(element: HTMLElement, view: EditorView, anchor: number): void {
  // 落点存在 dataset 上，文档在前方变化时由 updateDOM 就地更新，无需重建元素
  element.dataset.anchor = String(anchor);
  element.addEventListener("mousedown", (event) => {
    if (event.button !== 0 || !view.state.facet(EditorView.editable)) return;
    event.preventDefault();
    view.dispatch({ selection: { anchor: Number(element.dataset.anchor) }, userEvent: "select.pointer" });
    view.focus();
  });
}

/** 行内公式部件。 */
export class InlineMathWidget extends WidgetType {
  constructor(
    private readonly tex: string,
    private readonly anchor: number
  ) {
    super();
  }

  /**
   * 判断两个部件是否等价。
   *
   * @param other 另一个部件
   * @returns 源码与位置相同时为 true
   */
  eq(other: InlineMathWidget) {
    return other.tex === this.tex && other.anchor === this.anchor;
  }

  /**
   * 构建行内公式元素。
   *
   * @param view 所属编辑器视图
   * @returns 公式容器
   */
  toDOM(view: EditorView) {
    const element = document.createElement("span");
    element.className = "cm-md-math-inline";
    element.dataset.tex = this.tex;
    renderTex(element, this.tex, false, view);
    bindReveal(element, view, this.anchor);
    return element;
  }

  /**
   * 源码不变、仅位置移动时复用已渲染的元素。
   *
   * @param dom 旧部件的元素
   * @returns 公式源码一致时为 true
   */
  updateDOM(dom: HTMLElement) {
    if (dom.dataset.tex !== this.tex) return false;
    dom.dataset.anchor = String(this.anchor);
    return true;
  }

  /**
   * 声明该部件自行处理事件。
   *
   * @returns 恒为 true，点击由部件内部处理
   */
  ignoreEvent() {
    return true;
  }
}

/**
 * 块级公式部件。
 *
 * 光标在公式外时整体替换源码；光标在公式内时作为预览挂在源码下方，
 * 两种用途共用一个部件，由 preview 区分样式与交互。
 */
export class BlockMathWidget extends WidgetType {
  constructor(
    private readonly tex: string,
    private readonly anchor: number,
    private readonly preview: boolean
  ) {
    super();
  }

  /**
   * 判断两个部件是否等价。
   *
   * @param other 另一个部件
   * @returns 源码、位置与用途相同时为 true
   */
  eq(other: BlockMathWidget) {
    return other.tex === this.tex && other.anchor === this.anchor && other.preview === this.preview;
  }

  /**
   * 构建块级公式元素。
   *
   * @param view 所属编辑器视图
   * @returns 公式容器
   */
  toDOM(view: EditorView) {
    const element = document.createElement("div");
    element.className = this.preview ? "cm-md-math-block is-preview" : "cm-md-math-block";
    element.dataset.tex = this.tex;
    const body = document.createElement("div");
    element.appendChild(body);
    if (this.tex.trim()) renderTex(body, this.tex, true, view);
    else body.textContent = "$$";
    if (!this.preview) bindReveal(element, view, this.anchor);
    return element;
  }

  /**
   * 源码与用途不变、仅位置移动时复用已渲染的元素。
   *
   * @param dom 旧部件的元素
   * @returns 可复用时为 true
   */
  updateDOM(dom: HTMLElement) {
    if (dom.dataset.tex !== this.tex || dom.classList.contains("is-preview") !== this.preview) return false;
    dom.dataset.anchor = String(this.anchor);
    return true;
  }

  /**
   * 渲染前的估算高度，减少首次测量时的滚动跳动。
   *
   * @returns 估算高度（像素）
   */
  get estimatedHeight() {
    return 56;
  }

  /**
   * 声明该部件自行处理事件。
   *
   * @returns 恒为 true，点击由部件内部处理
   */
  ignoreEvent() {
    return true;
  }
}
