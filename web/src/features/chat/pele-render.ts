let peleModule: Promise<typeof import("pele")> | null = null;

/**
 * 加载 Pele，同一页面只请求一次。
 *
 * @returns Pele 模块
 */
function loadPele(): Promise<typeof import("pele")> {
  peleModule ??= import("pele");
  return peleModule;
}

/**
 * 用 Pele 把 Mermaid 源码渲染成 SVG。
 *
 * @param source 图表源码
 * @param idPrefix 同一页上多张图的标识前缀
 * @returns SVG 或错误信息
 */
export async function renderPeleDiagram(source: string, idPrefix: string): Promise<{ svg: string } | { error: string }> {
  try {
    const { render } = await loadPele();
    const { svg } = render(source, {
      idPrefix,
      responsive: true,
      links: true,
      images: false
    });
    return { svg };
  } catch (reason) {
    const message = reason instanceof Error ? reason.message : String(reason);
    return { error: message };
  }
}
