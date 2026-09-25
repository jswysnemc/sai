import { ListTree, Pin, PinOff } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";
import { useI18n } from "../../../../features/i18n/use-i18n";
import type { OutlineHeading } from "./outline-model";
import { useOutlinePinned, writeOutlinePinned } from "./use-outline-pinned";
import "./markdown-outline.css";

type MarkdownOutlineProps = {
  headings: readonly OutlineHeading[];
  /** 视口顶部所属标题的下标，-1 表示位于首个标题之前 */
  activeIndex: number;
  /** 点击标题时回调，参数为标题起始偏移 */
  onSelect: (from: number) => void;
};

/** 轨道最多展示的刻度数，超出时按顺序截断，完整列表见悬停面板。 */
const RAIL_LIMIT = 48;

/**
 * 渲染文档大纲。
 *
 * 同一份结构有两种形态，由容器宽度与固定状态通过样式切换：
 * 宽度充足且已固定时为右侧常驻栏；否则收成右缘刻度轨道，悬停或聚焦时浮出完整列表。
 *
 * @param props 标题列表、当前标题下标与跳转回调
 * @returns 大纲视图；文档没有标题时不渲染
 */
export function MarkdownOutline({ headings, activeIndex, onSelect }: MarkdownOutlineProps) {
  const { t } = useI18n();
  const pinned = useOutlinePinned();
  const listRef = useRef<HTMLOListElement>(null);
  // 以文档中出现的最高级别为缩进基准，全文从二级标题开始时不会整体右移
  const baseLevel = useMemo(() => Math.min(...headings.map((item) => item.level)), [headings]);

  useEffect(() => {
    // 当前标题变化时让列表跟随；只滚动列表自身，不用 scrollIntoView 以免带动外层容器
    const list = listRef.current;
    const active = list?.querySelector<HTMLElement>("[aria-current='location']");
    if (!list || !active) return;
    // 列表自身是定位容器，offsetTop 即相对列表顶部的距离
    const top = active.offsetTop;
    const bottom = top + active.offsetHeight;
    if (top < list.scrollTop) list.scrollTop = top;
    else if (bottom > list.scrollTop + list.clientHeight) list.scrollTop = bottom - list.clientHeight;
  }, [activeIndex]);

  if (!headings.length) return null;
  const pinLabel = pinned ? t("Unpin outline", "取消固定大纲") : t("Pin outline", "固定大纲");

  return (
    <aside className={pinned ? "markdown-outline is-pinned" : "markdown-outline"} aria-label={t("Outline", "大纲")}>
      <div className="markdown-outline-rail" aria-hidden="true">
        {headings.slice(0, RAIL_LIMIT).map((item, index) => (
          <span
            key={`${item.from}-${index}`}
            className={index === activeIndex ? "is-active" : ""}
            style={{ width: `${Math.max(0.375, 1 - (item.level - baseLevel) * 0.1875)}rem` }}
          />
        ))}
      </div>
      <div className="markdown-outline-panel">
        <header>
          <ListTree size={13} />
          <span>{t("Outline", "大纲")}</span>
          <button
            type="button"
            className="markdown-outline-pin"
            onClick={() => writeOutlinePinned(!pinned)}
            aria-pressed={pinned}
            aria-label={pinLabel}
            title={pinLabel}
          >
            {pinned ? <PinOff size={13} /> : <Pin size={13} />}
          </button>
        </header>
        <ol ref={listRef}>
          {headings.map((item, index) => (
            <li key={`${item.from}-${index}`}>
              <button
                type="button"
                aria-current={index === activeIndex ? "location" : undefined}
                style={{ paddingLeft: `calc(var(--space-xs) + ${item.level - baseLevel} * 0.75rem)` }}
                onClick={() => onSelect(item.from)}
                title={item.text}
              >
                {item.text}
              </button>
            </li>
          ))}
        </ol>
      </div>
    </aside>
  );
}
