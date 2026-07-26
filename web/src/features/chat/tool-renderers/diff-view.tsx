import { SyntaxHighlighter } from "../syntax-highlighter";
import { parseDiff } from "./diff-parser";
import type { DiffFile, DiffLine } from "./diff-parser";
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
  const { locale, t } = useI18n();
  const files = parseDiff(source, locale);
  if (files.length === 0) return null;
  return (
    <div className={`structured-diff${hideHeader ? " is-compact" : ""}`} role="region" aria-label={t("File diff", "文件差异")}>
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
function DiffFileBlock({ file, hideHeader, hidePath }: { file: DiffFile; hideHeader: boolean; hidePath: boolean }) {
  const { t } = useI18n();
  const showOldLine = file.lines.some((line) => line.oldLine !== undefined);
  const showNewLine = file.lines.some((line) => line.newLine !== undefined);
  const gutterClass = showOldLine && showNewLine ? "double-gutter" : showOldLine || showNewLine ? "single-gutter" : "no-gutter";
  // hideHeader 时完全不渲染路径/动作条，只保留 diff 行
  const showHead = !hideHeader;
  return (
    <section className="diff-file">
      {showHead && (
        <header className="diff-file-head">
          {!hidePath && file.path && <ToolFileReference path={file.path} />}
          {!file.path && <strong>{t("Change fragment", "变更片段")}</strong>}
          <small>{file.action}</small>
          <span className="diff-file-stats">
            {file.added > 0 && <b>+{file.added}</b>}
            {file.removed > 0 && <i>-{file.removed}</i>}
          </span>
        </header>
      )}
      <div className={`diff-file-lines ${gutterClass}`}>
        {file.lines.map((line, index) => (
          <DiffLineRow line={line} language={languageOfPath(file.path)} showOldLine={showOldLine} showNewLine={showNewLine} key={index} />
        ))}
      </div>
    </section>
  );
}

/**
 * 渲染一行差异内容，删除行显示旧行号、新增行显示新行号。
 *
 * @param props 解析后的差异行
 * @returns 差异行元素
 */
function DiffLineRow({ line, language, showOldLine, showNewLine }: { line: DiffLine; language?: string; showOldLine: boolean; showNewLine: boolean }) {
  // 1. 内容行渲染按需行号列与带标记的代码
  const marker = line.kind === "added" ? "+" : line.kind === "removed" ? "-" : " ";
  return (
    <div className={`diff-row ${line.kind}`}>
      {showOldLine && <span className="diff-gutter">{line.oldLine ?? ""}</span>}
      {showNewLine && <span className="diff-gutter">{line.newLine ?? ""}</span>}
      <code>
        <span className="diff-marker">{marker}</span>
        {line.text && language ? <SyntaxHighlighter language={language} source={line.text} /> : line.text || " "}
      </code>
    </div>
  );
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
