import type { DiffLayout } from "../diff-view";
import type { DiffLineHighlight } from "./diff-highlight";
import type { DiffLine } from "./diff-model";
import { buildSideBySide } from "./side-by-side";

type DiffCodeRowsProps = {
  lines: DiffLine[];
  layout: DiffLayout;
  highlights: ReadonlyMap<DiffLine, DiffLineHighlight>;
};

/**
 * 渲染统一或并排代码行，两侧共享网格行高，换行后仍保持对应关系。
 * @param props 代码行、布局与安全着色结果
 * @returns 带旧、新行号的差异行
 */
export function DiffCodeRows({ lines, layout, highlights }: DiffCodeRowsProps) {
  if (layout === "side") {
    return buildSideBySide(lines).map((row, index) => (
      <div className="review-diff-side-row" key={index}>
        <DiffCodeCell line={row.left} side="old" highlights={highlights} />
        <DiffCodeCell line={row.right} side="new" highlights={highlights} />
      </div>
    ));
  }
  return lines.map((line, index) => (
    <div className={`review-diff-row ${line.kind}`} key={index}>
      <span className="review-diff-number" aria-hidden>{line.oldLine}</span>
      <span className="review-diff-number" aria-hidden>{line.newLine}</span>
      <span className="review-diff-sign" aria-hidden>{line.kind === "added" ? "+" : line.kind === "removed" ? "-" : " "}</span>
      <DiffCodeText line={line} html={highlights.get(line)?.[line.kind === "removed" ? "old" : "new"]} />
    </div>
  ));
}

/**
 * 渲染并排视图中的一侧；缺失行保留空格位置，避免增删数量不同造成错位。
 * @param props 当前行、所属版本及着色结果
 * @returns 一侧的行号和代码单元格
 */
function DiffCodeCell({ line, side, highlights }: {
  line: DiffLine | null;
  side: "old" | "new";
  highlights: ReadonlyMap<DiffLine, DiffLineHighlight>;
}) {
  return (
    <div className={`review-diff-cell ${line?.kind ?? "is-empty"}`}>
      <span className="review-diff-number" aria-hidden>{side === "old" ? line?.oldLine : line?.newLine}</span>
      <span className="review-diff-sign" aria-hidden>{line?.kind === "added" ? "+" : line?.kind === "removed" ? "-" : " "}</span>
      {line && <DiffCodeText line={line} html={highlights.get(line)?.[side]} />}
    </div>
  );
}

/**
 * 输出已转义的语法 HTML；未生成着色结果时由 React 转义原始文本。
 * @param props 原始代码行及安全着色标记
 * @returns 保留空行高度的代码元素
 */
function DiffCodeText({ line, html }: { line: DiffLine; html?: string }) {
  return html === undefined
    ? <code className="review-diff-code">{line.text || " "}</code>
    : <code className="review-diff-code" dangerouslySetInnerHTML={{ __html: html || " " }} />;
}
