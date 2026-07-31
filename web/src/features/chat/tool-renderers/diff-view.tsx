import { memo, useMemo } from "react";
import { SyntaxHighlighter } from "../syntax-highlighter";
import { diffStatusLabel } from "./diff/diff-model";
import type { DiffFile, DiffLine } from "./diff/diff-model";
import { parseDiff } from "./diff/diff-parser";
import { ToolFileReference } from "./tool-file-reference";
import { useI18n } from "../../i18n/use-i18n";
import "./diff-view.css";

type DiffViewProps = {
  source: string;
  headerPath?: string;
  /** 为 true 时隐藏文件头，避免与外层文件行重复 */
  hideHeader?: boolean;
};

/**
 * 以 IDE 风格渲染统一 Diff 或 Codex patch 文本。
 *
 * @param props Diff 源文本
 * @returns 按文件分块、带双行号列的 Diff 视图
 */
export function DiffView({ source, headerPath, hideHeader = false }: DiffViewProps) {
  const { t } = useI18n();
  // 解析与字符级配对是纯计算，父组件重渲染时不应重跑
  const files = useMemo(() => parseDiff(source), [source]);
  if (files.length === 0) return null;
  return (
    <div
      className={`structured-diff${hideHeader ? " is-compact" : ""}`}
      role="region"
      aria-label={t("File diff", "文件差异")}
    >
      {files.map((file, index) => (
        <DiffFileBlock
          file={file}
          hideHeader={hideHeader || (files.length === 1 && file.path === headerPath)}
          hidePath={files.length === 1 && file.path === headerPath}
          key={`${file.path}-${index}`}
        />
      ))}
    </div>
  );
}

/**
 * 渲染单个文件的差异块，含文件名条与增删统计徽标。
 *
 * @param props 解析后的文件差异
 * @returns 文件差异块
 */
function DiffFileBlock({
  file,
  hideHeader,
  hidePath
}: {
  file: DiffFile;
  hideHeader: boolean;
  hidePath: boolean;
}) {
  const { t } = useI18n();
  const status = diffStatusLabel(file.status);
  const showOldLine = file.lines.some((line) => line.oldLine !== undefined);
  const showNewLine = file.lines.some((line) => line.newLine !== undefined);
  const gutterClass =
    showOldLine && showNewLine
      ? "double-gutter"
      : showOldLine || showNewLine
        ? "single-gutter"
        : "no-gutter";
  const showHead = !hideHeader;
  return (
    <section className="diff-file">
      {showHead && (
        <header className="diff-file-head">
          {!hidePath && file.path && <ToolFileReference path={file.path} />}
          {!file.path && <strong>{t("Change fragment", "变更片段")}</strong>}
          <small>{t(status.en, status.zh)}</small>
          <span className="diff-file-stats">
            {file.added > 0 && <b>+{file.added}</b>}
            {file.removed > 0 && <i>-{file.removed}</i>}
          </span>
        </header>
      )}
      {file.oldPath && (
        <p className="diff-file-note">
          {t(`Renamed from ${file.oldPath}`, `由 ${file.oldPath} 重命名`)}
        </p>
      )}
      {file.status === "binary" && (
        <p className="diff-file-note">{t("Binary file not shown", "二进制文件不展示内容")}</p>
      )}
      {file.lines.length > 0 && (
        <div className={`diff-file-lines ${gutterClass}`}>
          {file.lines.map((line, index) => (
            <DiffLineRow
              line={line}
              language={languageOfPath(file.path)}
              showOldLine={showOldLine}
              showNewLine={showNewLine}
              key={index}
            />
          ))}
        </div>
      )}
    </section>
  );
}

/**
 * 渲染一行差异内容，删除行显示旧行号、新增行显示新行号。
 *
 * @param props 解析后的差异行
 * @returns 差异行元素
 */
const DiffLineRow = memo(function DiffLineRow({
  line,
  language,
  showOldLine,
  showNewLine
}: {
  line: DiffLine;
  language?: string;
  showOldLine: boolean;
  showNewLine: boolean;
}) {
  // hunk 边界与无换行标记都不是代码，占满整行提示即可
  if (line.kind === "hunk" || line.kind === "no-newline") {
    return <div className={`diff-row ${line.kind}`}>{line.text}</div>;
  }
  const marker = line.kind === "added" ? "+" : line.kind === "removed" ? "-" : " ";
  return (
    <div className={`diff-row ${line.kind}`}>
      {showOldLine && <span className="diff-gutter">{line.oldLine ?? ""}</span>}
      {showNewLine && <span className="diff-gutter">{line.newLine ?? ""}</span>}
      <code>
        <span className="diff-marker">{marker}</span>
        <DiffLineContent line={line} language={language} />
      </code>
    </div>
  );
});

/**
 * 渲染行内容：已配对的行按字符级差异标出改动区间。
 *
 * @param props 差异行与着色语言
 * @returns 行内容元素
 */
function DiffLineContent({ line, language }: { line: DiffLine; language?: string }) {
  // 有字符级分段时优先展示改动区间，语法着色让位于差异定位
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

/**
 * 从文件路径推断代码着色语言。
 *
 * @param path 文件路径
 * @returns 扩展名语言标识，无扩展名时为 undefined
 */
function languageOfPath(path: string): string | undefined {
  const name = path.split("/").pop() ?? "";
  return name.includes(".") ? name.split(".").pop() : undefined;
}
