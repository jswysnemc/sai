import { ChevronDown, ChevronUp, Ellipsis, FoldVertical, UnfoldVertical } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import { Button } from "../../../../shared/ui/button/button";
import { useI18n } from "../../../i18n/use-i18n";
import type { DiffLayout } from "../diff-view";
import { buildDiffCodeBlocks } from "./diff-code-blocks";
import { DiffCodeRows } from "./diff-code-rows";
import { highlightDiffLines } from "./diff-highlight";
import type { DiffFile } from "./diff-model";
import "./diff-code-view.css";

type DiffCodeViewProps = {
  file: DiffFile;
  language?: string;
  layout: DiffLayout;
  wrap?: boolean;
};

const CONTEXT_LINES = 3;

/**
 * 渲染适合工作区审阅的代码差异，共用行号、字符着色、上下文折叠与变更导航。
 * @param props 文件差异、语言、统一或并排布局及换行设置
 * @returns 可用 F7、Shift+F7 跳转变更的差异正文
 */
export function DiffCodeView({ file, language, layout, wrap = true }: DiffCodeViewProps) {
  const { t } = useI18n();
  const blocks = useMemo(() => buildDiffCodeBlocks(file.lines), [file.lines]);
  const highlights = useMemo(() => highlightDiffLines(file.lines, language), [file.lines, language]);
  const changes = useMemo(() => blocks.flatMap((block, index) => block.kind === "change" ? [index] : []), [blocks]);
  const foldable = useMemo(() => blocks.flatMap((block, index) =>
    block.kind === "context" && block.lines.length > CONTEXT_LINES * 2 ? [index] : []), [blocks]);
  const [unfolded, setUnfolded] = useState<ReadonlySet<number>>(new Set());
  const [current, setCurrent] = useState(0);
  const changeRefs = useRef(new Map<number, HTMLDivElement>());
  const ordinal = Math.min(current, Math.max(0, changes.length - 1));
  const allExpanded = foldable.length > 0 && foldable.every((index) => unfolded.has(index));
  const lastLine = file.lines.reduce((max, line) => Math.max(max, line.oldLine ?? 0, line.newLine ?? 0), 1);
  const style = { "--review-line-digits": `${Math.max(3, String(lastLine).length) + 1}ch` } as CSSProperties;

  useEffect(() => {
    setUnfolded(new Set());
    setCurrent(0);
  }, [file.lines]);

  /**
   * 定位相邻变更块，索引始终约束在当前文件范围内。
   * @param next 目标变更序号，从零开始
   * @returns 无返回值
   */
  const goToChange = (next: number) => {
    const index = Math.max(0, Math.min(next, changes.length - 1));
    setCurrent(index);
    changeRefs.current.get(changes[index])?.scrollIntoView({ block: "nearest" });
  };

  /**
   * 处理差异区域内的变更跳转快捷键。
   * @param event 当前区域键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "F7" || !changes.length) return;
    event.preventDefault();
    event.stopPropagation();
    goToChange(ordinal + (event.shiftKey ? -1 : 1));
  };

  /**
   * 展开或折叠补丁实际包含的未修改代码。
   * @param index 上下文块序号
   * @returns 无返回值
   */
  const toggleContext = (index: number) => {
    setUnfolded((currentSet) => {
      const next = new Set(currentSet);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  };

  return (
    <div className={`review-diff${wrap ? " is-wrapped" : ""}`} style={style}
      data-layout={layout} tabIndex={0} onKeyDown={handleKeyDown}
      aria-label={t(`Changes in ${file.path}`, `${file.path} 的代码变更`)}>
      <div className="review-diff-toolbar">
        <span>{t(`${changes.length} changes`, `${changes.length} 处变更`)}</span>
        <div className="review-diff-navigation" role="group" aria-label={t("Navigate changes", "变更导航")}>
          <Button variant="ghost" size="icon" disabled={ordinal === 0} onClick={() => goToChange(ordinal - 1)}
            aria-label={t("Previous change", "上一处变更")} title={t("Previous change (Shift+F7)", "上一处变更（Shift+F7）")}>
            <ChevronUp size={13} />
          </Button>
          <span className="review-diff-position">{changes.length ? ordinal + 1 : 0}/{changes.length}</span>
          <Button variant="ghost" size="icon" disabled={ordinal >= changes.length - 1} onClick={() => goToChange(ordinal + 1)}
            aria-label={t("Next change", "下一处变更")} title={t("Next change (F7)", "下一处变更（F7）")}>
            <ChevronDown size={13} />
          </Button>
          {foldable.length > 0 && <Button variant="ghost" size="icon" aria-pressed={allExpanded}
            onClick={() => setUnfolded(allExpanded ? new Set() : new Set(foldable))}
            aria-label={allExpanded ? t("Fold context", "折叠上下文") : t("Expand context", "展开上下文")}
            title={allExpanded ? t("Fold context", "折叠上下文") : t("Expand context", "展开上下文")}>
            {allExpanded ? <FoldVertical size={13} /> : <UnfoldVertical size={13} />}
          </Button>}
        </div>
      </div>
      <div className={`review-diff-columns ${layout}`} aria-hidden>
        <span title={t("Before", "修改前")}>{layout === "side" ? t("Before", "修改前") : t("Old", "旧")}</span>
        <span title={t("After", "修改后")}>{layout === "side" ? t("After", "修改后") : t("New", "新")}</span>
      </div>
      <div className="review-diff-lines">
        <div className="review-diff-table">
          {blocks.map((block, index) => {
            if (block.kind === "marker") {
              const line = block.lines[0];
              return <div className="review-diff-gap" key={index}>
                <Ellipsis size={13} aria-hidden />
                <span>{line.kind === "no-newline" ? t("No newline at end of file", "文件末尾没有换行")
                  : line.foldedCount ? t(`${line.foldedCount} lines omitted`, `未显示 ${line.foldedCount} 行`)
                    : t("Next section", "下一处区段")}</span>
                {line.kind === "hunk" && <code>{line.text.replace(/^@@.*?@@\s*/u, "")}</code>}
              </div>;
            }
            const hiddenCount = block.kind === "context" ? Math.max(0, block.lines.length - CONTEXT_LINES * 2) : 0;
            const folded = hiddenCount > 0 && !unfolded.has(index);
            const changeOrdinal = changes.indexOf(index);
            return <div key={index} className={`review-diff-block ${block.kind}`}
              data-change={block.kind === "change" ? changeOrdinal : undefined}
              data-active={block.kind === "change" && ordinal === changeOrdinal ? "true" : undefined}
              onClick={block.kind === "change" ? () => setCurrent(changeOrdinal) : undefined}
              ref={(element) => {
                if (element) changeRefs.current.set(index, element);
                else changeRefs.current.delete(index);
              }}>
              <DiffCodeRows lines={folded ? block.lines.slice(0, CONTEXT_LINES) : block.lines} layout={layout} highlights={highlights} />
              {hiddenCount > 0 && <Button variant="ghost" className="review-diff-fold" aria-expanded={!folded} onClick={() => toggleContext(index)}>
                {folded ? <ChevronDown size={13} /> : <ChevronUp size={13} />}
                {folded ? t(`Show ${hiddenCount} unchanged lines`, `展开 ${hiddenCount} 行上下文`) : t("Fold context", "折叠上下文")}
              </Button>}
              {folded && <DiffCodeRows lines={block.lines.slice(-CONTEXT_LINES)} layout={layout} highlights={highlights} />}
            </div>;
          })}
        </div>
      </div>
    </div>
  );
}
