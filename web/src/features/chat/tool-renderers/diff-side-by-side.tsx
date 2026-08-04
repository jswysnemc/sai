import { memo, useMemo } from "react";
import type { DiffFile, DiffLine } from "./diff/diff-model";
import { buildSideBySide, type SideBySideRow } from "./diff/side-by-side";
import { SyntaxHighlighter } from "../syntax-highlighter";
import "./diff-view.css";

/**
 * 以左右两栏渲染单个文件的差异，删除在左、新增在右，块内按行对齐。
 *
 * 与统一视图共用同一份解析结果与字符级分段，只是布局不同：
 * 配对行同行对照，未配对的行在对侧留空槽。
 *
 * @param props 解析后的文件差异与着色语言
 * @returns 并排差异块
 */
export function DiffSideBySideBlock({ file, language }: { file: DiffFile; language?: string }) {
  const rows = useMemo(() => buildSideBySide(file.lines), [file.lines]);
  return (
    <div className="diff-side-grid" role="table" aria-label="side by side diff">
      {rows.map((row, index) => (
        <SideRow row={row} language={language} key={index} />
      ))}
    </div>
  );
}

/**
 * 渲染并排视图的一行：左右两格，缺失侧渲染空槽。
 *
 * @param props 对齐后的行与着色语言
 * @returns 一行两格的差异元素
 */
function SideRow({ row, language }: { row: SideBySideRow; language?: string }) {
  // hunk 与 no-newline 标记整行横跨两栏
  if (row.left && (row.left.kind === "hunk" || row.left.kind === "no-newline")) {
    return <div className={`diff-row ${row.left.kind} diff-span`}>{row.left.text}</div>;
  }
  return (
    <>
      <SideCell line={row.left} side="left" language={language} />
      <SideCell line={row.right} side="right" language={language} />
    </>
  );
}

/**
 * 渲染并排视图的单个格子。
 *
 * @param props 行内容、所属侧与着色语言；行缺失时渲染空槽
 * @returns 单格差异元素
 */
const SideCell = memo(function SideCell({
  line,
  side,
  language
}: {
  line: DiffLine | null;
  side: "left" | "right";
  language?: string;
}) {
  if (!line) {
    return <div className={`diff-row empty ${side}`} aria-hidden />;
  }
  const marker = line.kind === "added" ? "+" : line.kind === "removed" ? "-" : " ";
  return (
    <div className={`diff-row ${line.kind} ${side}`}>
      <span className="diff-gutter">{side === "left" ? (line.oldLine ?? "") : (line.newLine ?? "")}</span>
      <code>
        <span className="diff-marker">{marker}</span>
        <SideContent line={line} language={language} />
      </code>
    </div>
  );
});

/**
 * 渲染格内文本：有字符级分段时标出改动区间，否则语法着色。
 *
 * @param props 差异行与着色语言
 * @returns 行内容元素
 */
function SideContent({ line, language }: { line: DiffLine; language?: string }) {
  if (line.segments && line.segments.length > 0) {
    return (
      <>
        {line.segments.map((segment, index) =>
          segment.changed ? (
            <mark className="diff-inline" key={index}>
              {segment.text}
            </mark>
          ) : (
            <span key={index}>{segment.text}</span>
          )
        )}
      </>
    );
  }
  if (line.text && language) {
    return <SyntaxHighlighter language={language} source={line.text} />;
  }
  return <>{line.text || " "}</>;
}
