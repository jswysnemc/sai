import type { EditorState } from "@codemirror/state";
import type { SyntaxNode, Tree } from "@lezer/common";

/**
 * 光标触及时显露语法标记的行内节点。
 *
 * 行为对齐 Typora：光标进入或紧贴加粗、链接、行内代码等元素时，
 * 只显露该元素自己的标记（淡色），其余排版保持不变；离开后重新隐藏。
 * 显露范围限于单个行内元素，不会像整行切回源码那样引起大幅跳动。
 */
export const REVEALABLE_INLINE = new Set([
  "Emphasis",
  "StrongEmphasis",
  "InlineCode",
  "Strikethrough",
  "Link",
  "Image",
  "InlineMath",
  "Autolink",
]);

/**
 * 生成节点的范围键。
 *
 * @param from 节点起点
 * @param to 节点终点
 * @returns 可放进 Set 的键
 */
export function rangeKey(from: number, to: number): string {
  return `${from}:${to}`;
}

/**
 * 收集应显露标记的行内节点。
 *
 * 只看折叠的光标：拖选一大段时若逐个显露，会让整段文字宽度抖动。
 *
 * @param state 编辑器状态
 * @param tree 语法树
 * @returns 应显露节点的范围键集合
 */
export function revealedInlineNodes(state: EditorState, tree: Tree): Set<string> {
  const revealed = new Set<string>();
  for (const range of state.selection.ranges) {
    if (!range.empty) continue;
    // 光标两侧各解析一次，紧贴元素左右边界时同样视为触及
    for (const side of [-1, 1] as const) {
      for (let node: SyntaxNode | null = tree.resolveInner(range.head, side); node; node = node.parent) {
        if (REVEALABLE_INLINE.has(node.name) && node.from <= range.head && range.head <= node.to) {
          revealed.add(rangeKey(node.from, node.to));
        }
      }
    }
  }
  return revealed;
}
