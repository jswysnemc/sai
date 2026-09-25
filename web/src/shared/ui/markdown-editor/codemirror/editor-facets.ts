import { Facet } from "@codemirror/state";

/** 相对图片地址的解析函数，返回 null 表示无法加载。 */
export type ImageUrlResolver = (src: string) => string | null;

/**
 * 缺省的图片地址解析：只放行可直接加载的绝对地址与内联数据。
 *
 * 工作区里的相对路径没有可靠的基准目录，渲染出来只会是碎图，
 * 那种情况保留源码文本更有用；调用方可注入自己的解析函数。
 *
 * @param src 文档中的图片地址
 * @returns 可加载的地址，无法解析时为 null
 */
export function defaultImageUrl(src: string): string | null {
  return /^(https?:\/\/|data:image\/|\/)/i.test(src) ? src : null;
}

/** 当前编辑器的图片地址解析函数，取最先提供的一个。 */
export const imageUrlResolver = Facet.define<ImageUrlResolver, ImageUrlResolver>({
  combine: (values) => values[0] ?? defaultImageUrl,
});

/** 当前是否为深色主题，供需要把配色烘焙进产物的部件（如 Mermaid）读取。 */
export const editorDarkTheme = Facet.define<boolean, boolean>({
  combine: (values) => values[0] ?? false,
});
