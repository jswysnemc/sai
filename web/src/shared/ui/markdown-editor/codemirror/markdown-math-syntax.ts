import { tags } from "@lezer/highlight";
import type { BlockContext, Element, Line, MarkdownConfig } from "@lezer/markdown";

/** `$` 的字符码。 */
const DOLLAR = 36;

/** `\` 的字符码。 */
const BACKSLASH = 92;

/**
 * 判断字符码是否为空白。
 *
 * @param code 字符码，越界时为 -1
 * @returns 空格、制表符或换行时为 true
 */
function isSpace(code: number): boolean {
  return code === 32 || code === 9 || code === 10 || code === 13;
}

/**
 * 判断字符码是否为数字。
 *
 * @param code 字符码
 * @returns 0 到 9 时为 true
 */
function isDigit(code: number): boolean {
  return code >= 48 && code <= 57;
}

/**
 * 解析块级公式 `$$ … $$`。
 *
 * 开栏必须位于行首（容器标记之后），闭栏为以 `$$` 结尾的行；
 * 单行 `$$ x $$` 也视为块级公式。未闭合时一直延伸到文末，
 * 与围栏代码块的处理方式一致，输入过程中不会反复闪烁。
 *
 * @param cx 块解析上下文
 * @param line 当前行
 * @returns 已解析为公式块时为 true
 */
function parseBlockMath(cx: BlockContext, line: Line): boolean {
  const text = line.text.slice(line.pos);
  if (!text.startsWith("$$")) return false;
  const from = cx.lineStart + line.pos;
  const children: Element[] = [cx.elt("BlockMathMark", from, from + 2)];
  // 1. 单行写法：开栏之后同一行内就以 $$ 收尾
  const trimmed = line.text.trimEnd();
  if (trimmed.length - line.pos >= 4 && trimmed.endsWith("$$")) {
    const closeFrom = cx.lineStart + trimmed.length - 2;
    children.push(cx.elt("BlockMathMark", closeFrom, closeFrom + 2));
    cx.nextLine();
    cx.addElement(cx.elt("BlockMath", from, closeFrom + 2, children));
    return true;
  }
  // 2. 多行写法：逐行向下找闭栏；lezer 未公开容器深度，未闭合时延伸到文末，与围栏代码块一致
  let end = cx.lineStart + line.text.length;
  while (cx.nextLine()) {
    const body = line.text.trimEnd();
    if (body.endsWith("$$") && body.length - line.pos >= 2) {
      const closeFrom = cx.lineStart + body.length - 2;
      children.push(cx.elt("BlockMathMark", closeFrom, closeFrom + 2));
      end = closeFrom + 2;
      cx.nextLine();
      break;
    }
    end = cx.lineStart + line.text.length;
  }
  cx.addElement(cx.elt("BlockMath", from, end, children));
  return true;
}

/**
 * Markdown 数学公式语法扩展。
 *
 * 行内 `$…$` 采用 Pandoc 规则以避开货币写法：开 `$` 后不能是空白，
 * 闭 `$` 前不能是空白、后不能紧跟数字，例如 `$5 与 $10` 不会被识别为公式。
 */
export const markdownMath: MarkdownConfig = {
  defineNodes: [
    { name: "InlineMath", style: tags.special(tags.string) },
    { name: "InlineMathMark", style: tags.processingInstruction },
    { name: "BlockMath", block: true, style: tags.special(tags.string) },
    { name: "BlockMathMark", style: tags.processingInstruction },
  ],
  parseInline: [
    {
      name: "InlineMath",
      parse(cx, next, pos) {
        if (next !== DOLLAR || cx.char(pos + 1) === DOLLAR || isSpace(cx.char(pos + 1))) return -1;
        for (let index = pos + 1; index < cx.end; index += 1) {
          const code = cx.char(index);
          if (code === BACKSLASH) {
            // 转义字符连同其后一个字符一起跳过
            index += 1;
            continue;
          }
          if (code !== DOLLAR) continue;
          if (isSpace(cx.char(index - 1)) || isDigit(cx.char(index + 1))) return -1;
          return cx.addElement(
            cx.elt("InlineMath", pos, index + 1, [
              cx.elt("InlineMathMark", pos, pos + 1),
              cx.elt("InlineMathMark", index, index + 1),
            ])
          );
        }
        return -1;
      },
      before: "Emphasis",
    },
  ],
  parseBlock: [
    {
      name: "BlockMath",
      parse: parseBlockMath,
      endLeaf: (_cx, line) => line.text.slice(line.pos).startsWith("$$"),
      before: "FencedCode",
    },
  ],
};
