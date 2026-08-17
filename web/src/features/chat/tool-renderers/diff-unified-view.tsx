import { useMemo, useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { SyntaxHighlighter } from "../syntax-highlighter";
import type { DiffFile, DiffLine } from "./diff/diff-model";
import { CONTEXT_MARGIN } from "./diff/diff-blocks";
import { useI18n } from "../../i18n/use-i18n";

type UnifiedSegment =
  | { kind: "context"; lines: DiffLine[] }
  | { kind: "change"; lines: DiffLine[] }
  | { kind: "marker"; line: DiffLine };

/**
 * 渲染主消息区使用的单栏统一 Diff。
 *
 * @param props 解析后的文件差异和语言标识
 * @returns 带单列行号、变更背景和上下文折叠的差异正文
 */
export function DiffUnifiedView({ file, language }: { file: DiffFile; language?: string }) {
  const { t } = useI18n();
  const segments = useMemo(() => segmentLines(file.lines), [file.lines]);
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(new Set());

  return (
    <div className="diff-file-lines diff-unified-lines">
      {segments.map((segment, index) => {
        if (segment.kind === "marker") {
          return <UnifiedMarker line={segment.line} key={`marker-${index}`} t={t} />;
        }
        if (segment.kind === "change") {
          return (
            <div className="diff-unified-change" key={`change-${index}`}>
              {segment.lines.map((line, lineIndex) => (
                <UnifiedLine line={line} language={language} key={`change-line-${lineIndex}`} />
              ))}
            </div>
          );
        }

        const foldCount = Math.max(segment.lines.length - CONTEXT_MARGIN * 2, 0);
        const isExpanded = expanded.has(index) || foldCount === 0;
        const visibleLines = isExpanded
          ? segment.lines
          : [...segment.lines.slice(0, CONTEXT_MARGIN), ...segment.lines.slice(-CONTEXT_MARGIN)];
        return (
          <div className="diff-unified-context" key={`context-${index}`}>
            {visibleLines.slice(0, isExpanded ? visibleLines.length : CONTEXT_MARGIN).map((line, lineIndex) => (
              <UnifiedLine line={line} language={language} key={`context-head-${lineIndex}`} />
            ))}
            {!isExpanded && (
              <button
                type="button"
                className="diff-unified-fold"
                onClick={() => setExpanded((current) => {
                  const next = new Set(current);
                  next.add(index);
                  return next;
                })}
                aria-label={t(`Show ${foldCount} unchanged lines`, `展开 ${foldCount} 行未修改内容`)}
              >
                <ChevronDown size={13} aria-hidden />
                <span>{t(`${foldCount} unchanged lines`, `${foldCount} 行未修改内容`)}</span>
              </button>
            )}
            {!isExpanded && visibleLines.slice(CONTEXT_MARGIN).map((line, lineIndex) => (
              <UnifiedLine line={line} language={language} key={`context-tail-${lineIndex}`} />
            ))}
            {isExpanded && foldCount > 0 && (
              <button
                type="button"
                className="diff-unified-fold"
                onClick={() => setExpanded((current) => {
                  const next = new Set(current);
                  next.delete(index);
                  return next;
                })}
                aria-label={t("Fold unchanged lines", "折叠未修改内容")}
              >
                <ChevronUp size={13} aria-hidden />
                <span>{t("Fold unchanged lines", "折叠未修改内容")}</span>
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}

/**
 * 将差异行按上下文、变更和 hunk 标记切分，保证折叠只影响未修改内容。
 *
 * @param lines 文件差异行
 * @returns 可独立渲染的统一视图片段
 */
function segmentLines(lines: DiffLine[]): UnifiedSegment[] {
  const segments: UnifiedSegment[] = [];
  let context: DiffLine[] = [];
  let change: DiffLine[] = [];
  const flushContext = () => {
    if (context.length > 0) segments.push({ kind: "context", lines: context });
    context = [];
  };
  const flushChange = () => {
    if (change.length > 0) segments.push({ kind: "change", lines: change });
    change = [];
  };

  for (const line of lines) {
    if (line.kind === "hunk" || line.kind === "no-newline") {
      flushChange();
      flushContext();
      segments.push({ kind: "marker", line });
    } else if (line.kind === "context") {
      flushChange();
      context.push(line);
    } else {
      flushContext();
      change.push(line);
    }
  }
  flushChange();
  flushContext();
  return segments;
}

/**
 * 渲染 hunk 或文件末尾无换行标记。
 *
 * @param props 标记行和本地化函数
 * @returns 弱化的差异分隔行
 */
function UnifiedMarker({
  line,
  t
}: {
  line: DiffLine;
  t: (english: string, chinese: string) => string;
}) {
  if (line.kind === "no-newline") {
    return <div className="diff-unified-marker">{line.text}</div>;
  }
  if (line.foldedCount) {
    return (
      <div className="diff-unified-fold diff-unified-hunk-fold" role="status">
        <ChevronDown size={13} aria-hidden />
        <span>{t(`${line.foldedCount} unchanged lines`, `${line.foldedCount} 行未修改内容`)}</span>
      </div>
    );
  }
  return null;
}

/**
 * 渲染统一 Diff 的单行，删除行使用旧行号，新增和上下文使用新行号。
 *
 * @param props 差异行和语言标识
 * @returns 单行差异元素
 */
function UnifiedLine({ line, language }: { line: DiffLine; language?: string }) {
  const lineNumber = line.kind === "removed" ? line.oldLine : line.newLine ?? line.oldLine;
  const marker = line.kind === "added" ? "+" : line.kind === "removed" ? "-" : " ";
  return (
    <div className={`diff-row diff-unified-row ${line.kind}`}>
      <span className="diff-gutter">{lineNumber ?? ""}</span>
      <code>
        <span className="diff-marker">{marker}</span>
        <span className="diff-code-content">
          <DiffLineContent line={line} language={language} />
        </span>
      </code>
    </div>
  );
}

/**
 * 渲染单行文本并保留字符级差异标记。
 *
 * @param props 差异行和语言标识
 * @returns 代码正文
 */
function DiffLineContent({ line, language }: { line: DiffLine; language?: string }) {
  if (line.segments && line.segments.length > 0) {
    return (
      <>
        {line.segments.map((segment, index) => segment.changed
          ? <mark className="diff-inline" key={index}>{segment.text}</mark>
          : <span key={index}>{segment.text}</span>)}
      </>
    );
  }
  if (line.text && language) return <SyntaxHighlighter language={language} source={line.text} />;
  return <>{line.text || " "}</>;
}
