import type { CSSProperties } from "react";

/**
 * 【代码块】【行号列】按总行数给出行号位数，供 CSS 计算定宽行号列。
 *
 * 行号列宽度固定为「最大行号位数 × 1ch + 内边距」，
 * 第 9 行与第 10 行的正文起点保持一致，目录树连接线不会错位。
 *
 * @param lineCount 代码块总行数
 * @returns 写入 `--line-number-digits` 的内联样式；至少 2 位
 */
export function lineNumberDigitsStyle(lineCount: number): CSSProperties {
  const digits = Math.max(2, String(Math.max(1, lineCount)).length);
  return { "--line-number-digits": digits } as CSSProperties;
}
