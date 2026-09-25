import { EditorView, WidgetType } from "@codemirror/view";

/** 渲染结果缓存上限，超出时丢弃最旧条目。 */
const CACHE_LIMIT = 30;

/** 编辑源码时重绘预览的防抖间隔（毫秒）。 */
const PREVIEW_DEBOUNCE_MS = 260;

/** 模块级缓存：键为主题加源码，值为 SVG 或错误信息。 */
const cache = new Map<string, { svg: string } | { error: string }>();

/** mermaid.initialize 是全局配置，渲染必须串行，避免不同主题的调用互相覆盖。 */
let queue: Promise<unknown> = Promise.resolve();

/** 渲染序号，保证每次调用 mermaid.render 的元素 id 唯一。 */
let sequence = 0;

/**
 * 串行渲染一张 Mermaid 图表并缓存结果。
 *
 * @param source 图表源码
 * @param dark 是否为深色主题
 * @returns SVG 或错误信息
 */
function renderMermaid(source: string, dark: boolean): Promise<{ svg: string } | { error: string }> {
  const key = `${dark ? "dark" : "light"}\u0000${source}`;
  const cached = cache.get(key);
  if (cached) return Promise.resolve(cached);
  const task = queue.then(async () => {
    const { default: mermaid } = await import("mermaid");
    mermaid.initialize({ startOnLoad: false, theme: dark ? "dark" : "neutral", securityLevel: "strict" });
    sequence += 1;
    try {
      const { svg } = await mermaid.render(`sai-md-mermaid-${sequence}`, source);
      return { svg };
    } catch (reason) {
      return { error: reason instanceof Error ? reason.message : String(reason) };
    }
  });
  queue = task.catch(() => undefined);
  return task.then((result) => {
    cache.set(key, result);
    if (cache.size > CACHE_LIMIT) cache.delete(cache.keys().next().value as string);
    return result;
  });
}

/**
 * 把渲染结果写入元素。
 *
 * @param element 图表容器
 * @param result SVG 或错误信息
 * @returns 无
 */
function paint(element: HTMLElement, result: { svg: string } | { error: string }): void {
  if ("svg" in result) {
    // 1. mermaid 以 strict 安全级别输出，SVG 内不含脚本与事件属性
    element.innerHTML = result.svg;
    element.classList.remove("is-error");
  } else {
    element.textContent = result.error;
    element.classList.add("is-error");
  }
  element.classList.remove("is-pending");
}

/** 各容器最近一次请求的源码，防止慢请求覆盖新结果。 */
const latest = new WeakMap<HTMLElement, string>();

/** 各容器的防抖计时器。 */
const timers = new WeakMap<HTMLElement, ReturnType<typeof setTimeout>>();

/**
 * 安排一次重绘；编辑中的预览做防抖，旧图保留到新图就绪，避免闪烁。
 *
 * @param element 图表容器
 * @param source 图表源码
 * @param dark 是否为深色主题
 * @param view 所属编辑器视图
 * @param delay 防抖延迟（毫秒）
 * @returns 无
 */
function schedule(element: HTMLElement, source: string, dark: boolean, view: EditorView, delay: number): void {
  latest.set(element, source);
  clearTimeout(timers.get(element));
  timers.set(
    element,
    setTimeout(() => {
      void renderMermaid(source, dark).then((result) => {
        if (!element.isConnected || latest.get(element) !== source) return;
        paint(element, result);
        view.requestMeasure();
      });
    }, delay)
  );
}

/**
 * Mermaid 图表部件。
 *
 * 光标在代码块外时整体替换源码；光标在块内时作为实时预览挂在源码下方。
 */
export class MermaidWidget extends WidgetType {
  constructor(
    private readonly source: string,
    private readonly dark: boolean,
    private readonly anchor: number,
    private readonly preview: boolean
  ) {
    super();
  }

  /**
   * 判断两个部件是否等价。
   *
   * @param other 另一个部件
   * @returns 源码、主题、位置与用途相同时为 true
   */
  eq(other: MermaidWidget) {
    return (
      other.source === this.source &&
      other.dark === this.dark &&
      other.anchor === this.anchor &&
      other.preview === this.preview
    );
  }

  /**
   * 构建图表容器并发起渲染。
   *
   * @param view 所属编辑器视图
   * @returns 图表容器
   */
  toDOM(view: EditorView) {
    const element = document.createElement("div");
    element.className = this.preview ? "cm-md-mermaid is-preview is-pending" : "cm-md-mermaid is-pending";
    this.bind(element, view);
    schedule(element, this.source, this.dark, view, 0);
    return element;
  }

  /**
   * 复用已有容器：只更新源码并防抖重绘，旧图保留到新图就绪。
   *
   * @param dom 旧部件的容器
   * @param view 所属编辑器视图
   * @returns 用途一致时为 true，表示已就地更新
   */
  updateDOM(dom: HTMLElement, view: EditorView) {
    if (dom.classList.contains("is-preview") !== this.preview) return false;
    dom.dataset.anchor = String(this.anchor);
    schedule(dom, this.source, this.dark, view, this.preview ? PREVIEW_DEBOUNCE_MS : 0);
    return true;
  }

  /**
   * 非预览用途下点击图表进入源码编辑。
   *
   * @param element 图表容器
   * @param view 所属编辑器视图
   * @returns 无
   */
  private bind(element: HTMLElement, view: EditorView): void {
    element.dataset.anchor = String(this.anchor);
    if (this.preview) return;
    element.addEventListener("mousedown", (event) => {
      if (event.button !== 0 || !view.state.facet(EditorView.editable)) return;
      event.preventDefault();
      view.dispatch({ selection: { anchor: Number(element.dataset.anchor) }, userEvent: "select.pointer" });
      view.focus();
    });
  }

  /**
   * 渲染前的估算高度，减少首次测量时的滚动跳动。
   *
   * @returns 估算高度（像素）
   */
  get estimatedHeight() {
    return 200;
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
