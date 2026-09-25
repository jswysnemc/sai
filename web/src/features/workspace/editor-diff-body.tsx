import { ChevronUp, X } from "lucide-react";
import { useMemo, useState, type CSSProperties } from "react";
import { api } from "../../api/client";
import { highlightSource, splitHighlightedLines } from "../chat/syntax-highlighter";
import { contextLinesFromFile } from "../chat/tool-renderers/diff/diff-fold-lines";
import { highlightDiffLines } from "../chat/tool-renderers/diff/diff-highlight";
import type { DiffFile, DiffLine } from "../chat/tool-renderers/diff/diff-model";
import { useI18n } from "../i18n/use-i18n";
import { buildEditorDiffSegments, type EditorDiffSegment } from "./editor-diff-segments";

type EditorDiffBodyProps = {
  file: DiffFile;
  path: string;
  language?: string;
};

/**
 * 渲染编辑器里的统一差异：未修改行收成一条，改动行只留一列行号。
 *
 * @param props 解析后的文件、路径和语言
 * @returns 文件头和差异正文
 */
export function EditorDiffBody({ file, path, language }: EditorDiffBodyProps) {
  const { t } = useI18n();
  const segments = useMemo(() => buildEditorDiffSegments(file.lines), [file.lines]);
  const highlights = useMemo(() => highlightDiffLines(file.lines, language), [file.lines, language]);
  const [open, setOpen] = useState<ReadonlySet<number>>(new Set());
  const lastGap = segments.reduce((found, segment, index) => segment.kind === "gap" ? index : found, -1);
  const lastLine = file.lines.reduce((max, line) => Math.max(max, line.oldLine ?? 0, line.newLine ?? 0), 1);

  return (
    <div className="editor-diff-sheet" style={{ "--editor-diff-digits": `${Math.max(2, String(lastLine).length)}ch` } as CSSProperties}>
      <header className="editor-diff-head">
        <span className="editor-diff-hash" aria-hidden>#</span>
        <span className="editor-diff-path" title={path}>{path}</span>
        <span className="editor-diff-stats">
          {file.added > 0 && <b>+{file.added}</b>}
          {file.removed > 0 && <i>-{file.removed}</i>}
        </span>
      </header>
      {segments.map((segment, index) => segment.kind === "gap" ? (
        <EditorDiffGap
          key={index}
          path={path}
          language={language}
          segment={segment}
          trailing={index === lastGap}
          expanded={open.has(index)}
          label={t(`${segment.count} unmodified lines`, `${segment.count} 行未修改`)}
          onToggle={() => setOpen((current) => {
            const next = new Set(current);
            if (next.has(index)) next.delete(index);
            else next.add(index);
            return next;
          })}
        />
      ) : (
        <div className="editor-diff-change" key={index}>
          {segment.lines.map((item) => (
            <EditorDiffRow key={item.index} line={item.line} index={item.index} html={highlights.get(item.line)?.[item.line.kind === "removed" ? "old" : "new"]} />
          ))}
        </div>
      ))}
    </div>
  );
}

type EditorDiffGapProps = {
  path: string;
  language?: string;
  segment: Extract<EditorDiffSegment, { kind: "gap" }>;
  trailing: boolean;
  expanded: boolean;
  label: string;
  onToggle: () => void;
};

/**
 * 渲染一条可展开的未修改间隔。
 *
 * @param props 间隔内容、是否展开和文案
 * @returns 折叠条；展开后接上未修改行
 */
function EditorDiffGap({ path, language, segment, trailing, expanded, label, onToggle }: EditorDiffGapProps) {
  const { t } = useI18n();
  const [loaded, setLoaded] = useState<Map<DiffLine, DiffLine[]> | null>(null);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const Icon = expanded || trailing ? ChevronUp : X;

  /**
   * 展开时补上补丁里没有的行，再交给父级切换状态。
   */
  const toggle = async () => {
    const folds = segment.pieces.filter((piece) => piece.kind === "fold");
    if (!expanded && folds.length > 0 && !loaded && !failed) {
      setBusy(true);
      try {
        const content = (await api.workspace.file(path)).content;
        setLoaded(new Map(folds.map((piece) => [piece.line, contextLinesFromFile(content, piece.line)])));
      } catch {
        setFailed(true);
      } finally {
        setBusy(false);
      }
    }
    onToggle();
  };

  const text = failed
    ? t("Could not read these lines", "读不到这些未修改行")
    : busy
      ? t("Reading omitted lines", "正在读取未修改行")
      : label;

  return (
    <div className="editor-diff-gap-block">
      <button type="button" className="editor-diff-gap" aria-expanded={expanded} onClick={() => void toggle()}>
        <Icon size={13} aria-hidden />
        <span>{text}</span>
      </button>
      {expanded && <ExpandedGap language={language} segment={segment} loaded={loaded} />}
    </div>
  );
}

/**
 * 展开后按原顺序画出上下文行和回填行。
 *
 * @param props 间隔片段、语言和已读取的省略行
 * @returns 未修改代码行
 */
function ExpandedGap({ language, segment, loaded }: {
  language?: string;
  segment: Extract<EditorDiffSegment, { kind: "gap" }>;
  loaded: Map<DiffLine, DiffLine[]> | null;
}) {
  return segment.pieces.map((piece, index) => {
    if (piece.kind === "line") return <EditorDiffRow key={`line-${piece.index}`} line={piece.line} index={piece.index} />;
    const lines = loaded?.get(piece.line) ?? [];
    return <FoldLines key={`fold-${index}`} lines={lines} language={language} />;
  });
}

/**
 * 给回填的未修改行着色。
 *
 * @param props 回填行和语言
 * @returns 代码行列表
 */
function FoldLines({ lines, language }: { lines: DiffLine[]; language?: string }) {
  const html = useMemo(() => {
    if (lines.length === 0) return [];
    return splitHighlightedLines(highlightSource(lines.map((line) => line.text).join("\n"), language).value);
  }, [lines, language]);
  return lines.map((line, index) => <EditorDiffRow key={index} line={line} html={html[index]} />);
}

/**
 * 渲染一行差异。删除行用旧行号，其余用新行号。
 *
 * @param props 差异行、原文下标和可选着色 HTML
 * @returns 单行
 */
function EditorDiffRow({ line, index, html }: { line: DiffLine; index?: number; html?: string }) {
  if (line.kind === "no-newline") {
    return <div className="editor-diff-note">{line.text}</div>;
  }
  const number = line.kind === "removed" ? line.oldLine : line.newLine ?? line.oldLine;
  return (
    <div
      className={`editor-diff-row ${line.kind}`}
      data-diff-row=""
      data-diff-index={index}
      data-line-text={line.text}
    >
      <span className="editor-diff-gutter" aria-hidden>{number ?? ""}</span>
      {html === undefined
        ? <code>{line.text || " "}</code>
        : <code dangerouslySetInnerHTML={{ __html: html || " " }} />}
    </div>
  );
}
