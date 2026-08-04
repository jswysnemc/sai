import { useMemo, useRef, useState } from "react";
import { ChevronDown, ChevronUp, UnfoldVertical } from "lucide-react";
import type { DiffFile } from "./diff/diff-model";
import { buildSideBySide, type SideBySideRow } from "./diff/side-by-side";
import {
  CONTEXT_MARGIN,
  foldPlan,
  isContextRow,
  segmentRows,
  type RowSegment
} from "./diff/diff-blocks";
import { SyntaxHighlighter } from "../syntax-highlighter";
import { useI18n } from "../../i18n/use-i18n";
import "./diff-view.css";

/**
 * IDEA 式差异查看器：左右双栏、中间连接带、未改动区折叠、变更块导航。
 *
 * 与旧的简单并排视图的区别：
 * - 未改动的上下文默认折叠为可展开的折条，只保留变更块周围少量行
 * - 每个变更块中间有一条连接带，颜色按增/删/混合区分
 * - 顶部提供上一处/下一处变更导航与计数
 *
 * @param props 解析后的文件差异与着色语言
 * @returns IDEA 式差异查看器
 */
export function DiffIdeaView({ file, language }: { file: DiffFile; language?: string }) {
  const { t } = useI18n();
  const rows = useMemo(() => buildSideBySide(file.lines), [file.lines]);
  const segments = useMemo(() => segmentRows(rows), [rows]);
  const changeIndexes = useMemo(
    () => segments.map((segment, index) => (segment.kind === "change" ? index : -1)).filter((index) => index >= 0),
    [segments]
  );
  // 手动展开的上下文段；默认全部折叠
  const [unfolded, setUnfolded] = useState<ReadonlySet<number>>(new Set());
  const [current, setCurrent] = useState(0);
  const blockRefs = useRef<Record<number, HTMLDivElement | null>>({});

  const changeCount = changeIndexes.length;

  /** 展开或收起指定上下文段。 */
  const toggleFold = (index: number): void => {
    setUnfolded((prev) => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  };

  /** 跳转到第 n 个变更块并滚动到可见。 */
  const goToChange = (ordinal: number): void => {
    const clamped = Math.min(Math.max(ordinal, 0), Math.max(changeCount - 1, 0));
    setCurrent(clamped);
    const segmentIndex = changeIndexes[clamped];
    blockRefs.current[segmentIndex]?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  };

  if (changeCount === 0) {
    return <div className="diff-idea-empty">{t("No changes in this file", "该文件没有可显示的变更")}</div>;
  }

  return (
    <div className="diff-idea">
      <div className="diff-idea-toolbar">
        <span className="diff-idea-count">
          {t(`${changeCount} change blocks`, `${changeCount} 处变更`)}
        </span>
        <span className="diff-idea-nav">
          <button
            type="button"
            onClick={() => goToChange(current - 1)}
            disabled={current === 0}
            title={t("Previous change", "上一处变更")}
            aria-label={t("Previous change", "上一处变更")}
          >
            <ChevronUp size={13} />
          </button>
          <span>{current + 1} / {changeCount}</span>
          <button
            type="button"
            onClick={() => goToChange(current + 1)}
            disabled={current >= changeCount - 1}
            title={t("Next change", "下一处变更")}
            aria-label={t("Next change", "下一处变更")}
          >
            <ChevronDown size={13} />
          </button>
          <button
            type="button"
            onClick={() => setUnfolded(new Set(segments.map((_, index) => index)))}
            title={t("Expand all folded regions", "展开全部折叠区")}
            aria-label={t("Expand all folded regions", "展开全部折叠区")}
          >
            <UnfoldVertical size={13} />
          </button>
        </span>
      </div>
      <div className="diff-idea-body">
        {segments.map((segment, index) => (
          <SegmentBlock
            key={index}
            segment={segment}
            index={index}
            unfolded={unfolded.has(index)}
            language={language}
            onToggleFold={() => toggleFold(index)}
            refCallback={(element) => {
              blockRefs.current[index] = element;
            }}
            t={t}
          />
        ))}
      </div>
    </div>
  );
}

type SegmentBlockProps = {
  segment: RowSegment;
  index: number;
  unfolded: boolean;
  language?: string;
  onToggleFold: () => void;
  refCallback: (element: HTMLDivElement | null) => void;
  t: (en: string, zh: string) => string;
};

/**
 * 渲染一个段：上下文段可折叠，变更段带中间连接带。
 *
 * @param props 段内容、折叠状态与回调
 * @returns 段元素
 */
function SegmentBlock({ segment, index, unfolded, language, onToggleFold, refCallback, t }: SegmentBlockProps) {
  // 1. 上下文段：短段直接铺开，长段折叠首尾之外的中间
  if (segment.kind === "context") {
    const plan = foldPlan(segment.rows.length);
    const showAll = unfolded || plan.foldCount === 0;
    const head = showAll ? segment.rows : segment.rows.slice(0, plan.head);
    const tail = showAll ? [] : segment.rows.slice(segment.rows.length - plan.tail);
    return (
      <div className="diff-idea-context">
        {head.map((row, rowIndex) => (
          <IdeaRow row={row} language={language} key={`h${rowIndex}`} />
        ))}
        {!showAll && (
          <button type="button" className="diff-idea-fold" onClick={onToggleFold}>
            {t(`Show ${plan.foldCount} unchanged lines`, `展开 ${plan.foldCount} 行未改动内容`)}
          </button>
        )}
        {showAll && plan.foldCount > 0 && (
          <button type="button" className="diff-idea-fold" onClick={onToggleFold}>
            {t("Fold unchanged lines", "折叠未改动内容")}
          </button>
        )}
        {tail.map((row, rowIndex) => (
          <IdeaRow row={row} language={language} key={`t${rowIndex}`} />
        ))}
      </div>
    );
  }

  // 2. 变更段：按增/删/混合决定连接带颜色
  const hasLeft = segment.rows.some((row) => row.left && row.left.kind === "removed");
  const hasRight = segment.rows.some((row) => row.right && row.right.kind === "added");
  const tone = hasLeft && hasRight ? "mixed" : hasRight ? "added" : "removed";
  return (
    <div className={`diff-idea-change diff-tone-${tone}`} ref={refCallback}>
      <div className="diff-idea-cols">
        <div className="diff-idea-col left">
          {segment.rows.map((row, rowIndex) => (
            <IdeaCell line={row.left} side="left" language={language} key={rowIndex} />
          ))}
        </div>
        <div className="diff-idea-band" aria-hidden />
        <div className="diff-idea-col right">
          {segment.rows.map((row, rowIndex) => (
            <IdeaCell line={row.right} side="right" language={language} key={rowIndex} />
          ))}
        </div>
      </div>
    </div>
  );
}

/**
 * 渲染上下文段的一整行：左右同列。
 *
 * @param props 对齐行与着色语言
 * @returns 行元素
 */
function IdeaRow({ row, language }: { row: SideBySideRow; language?: string }) {
  // hunk 标记整行横跨
  if (row.left && (row.left.kind === "hunk" || row.left.kind === "no-newline")) {
    return <div className="diff-row hunk diff-span">{row.left.text}</div>;
  }
  return (
    <div className="diff-idea-rowline">
      <IdeaCell line={row.left} side="left" language={language} />
      <span className="diff-idea-band-flat" aria-hidden />
      <IdeaCell line={row.right} side="right" language={language} />
    </div>
  );
}

/**
 * 渲染单侧格子：行号 + 内容，缺失时留空槽。
 *
 * @param props 行内容、所属侧与着色语言
 * @returns 格子元素
 */
function IdeaCell({
  line,
  side,
  language
}: {
  line: import("./diff/diff-model").DiffLine | null;
  side: "left" | "right";
  language?: string;
}) {
  if (!line) return <div className={`diff-row empty ${side}`} aria-hidden />;
  const marker = line.kind === "added" ? "+" : line.kind === "removed" ? "-" : " ";
  return (
    <div className={`diff-row ${line.kind} ${side}`}>
      <span className="diff-gutter">{side === "left" ? (line.oldLine ?? "") : (line.newLine ?? "")}</span>
      <code>
        <span className="diff-marker">{marker}</span>
        <IdeaContent line={line} language={language} />
      </code>
    </div>
  );
}

/**
 * 渲染格内文本：字符级分段优先，否则语法着色。
 *
 * @param props 差异行与着色语言
 * @returns 内容元素
 */
function IdeaContent({ line, language }: { line: import("./diff/diff-model").DiffLine; language?: string }) {
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
