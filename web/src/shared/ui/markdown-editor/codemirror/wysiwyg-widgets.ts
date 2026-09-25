import { WidgetType } from "@codemirror/view";

/**
 * 内联图片部件。
 *
 * 地址由调用方经 imageUrlResolver 解析好再传入，部件本身不关心来源。
 * 加载失败时退化为带替代文本的灰色占位，避免留下破图标。
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
    image.decoding = "async";
    image.draggable = false;
    image.addEventListener("error", () => {
      wrap.classList.add("is-broken");
      wrap.textContent = this.alt || this.url;
    });
    wrap.appendChild(image);
    return wrap;
  }

  /**
   * 图片高度未知，交给浏览器测量后再同步给编辑器。
   *
   * @returns 估算高度（像素）
   */
  get estimatedHeight() {
    return 160;
  }
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
