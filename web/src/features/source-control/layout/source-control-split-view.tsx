import { ArrowLeft } from "lucide-react";
import { Children, useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { useI18n } from "../../i18n/use-i18n";
import {
  SOURCE_CONTROL_SPLIT_DEFAULT_WIDTH,
  SOURCE_CONTROL_SPLIT_MIN_DETAIL_WIDTH,
  SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH,
  shouldStackSourceControlSplit,
  useSourceControlSplitState
} from "./source-control-split-state";
import { useStackedPaneState } from "./stacked-pane-state";
import "./source-control-split-view.css";
import "./stacked-pane.css";

type SourceControlSplitViewProps = {
  className: string;
  /** 窄屏详情区标题，展示在返回栏上 */
  detailTitle?: string;
  /**
   * 详情侧的选中标识。窄屏下该值变化即视为用户选中了新条目，
   * 自动从列表切到详情；宽屏两侧同屏，无需切换。
   */
  detailKey?: string | null;
  children: ReactNode;
};

/**
 * 渲染可调整左右宽度的 Git 双栏视图，窄屏时收拢为单区域切换。
 *
 * 宽屏保持左右分栏并支持拖拽调宽；容器窄到放不下两侧最小宽度时，
 * 一次只呈现列表或详情，选中条目进入详情、返回键回到列表。
 *
 * @param props 外层类名、窄屏详情标题与左右两侧内容
 * @returns 带可访问分隔条的 Git 双栏布局，或窄屏堆叠布局
 */
export function SourceControlSplitView(props: SourceControlSplitViewProps) {
  const { t } = useI18n();
  const rootRef = useRef<HTMLDivElement>(null);
  const detailRef = useRef<HTMLDivElement>(null);
  const [stacked, setStacked] = useState(false);
  const { listWidth, resize, constrain } = useSourceControlSplitState();
  const { pane, direction, showList, showDetail } = useStackedPaneState();
  const [list, detail] = Children.toArray(props.children);

  useEffect(() => {
    const root = rootRef.current;
    if (!root || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      const width = entry.contentRect.width;
      const nextStacked = shouldStackSourceControlSplit(width);
      setStacked(nextStacked);
      if (!nextStacked) constrain(width);
    });
    observer.observe(root);
    return () => observer.disconnect();
  }, [constrain]);

  useEffect(() => {
    // 窄屏下选中新条目即进入详情；detailKey 为空表示回到无选中态
    if (!stacked) return;
    if (props.detailKey) showDetail();
    else showList();
  }, [props.detailKey, showDetail, showList, stacked]);

  useEffect(() => {
    // 进入详情时复位滚动，避免沿用上一个条目的偏移
    if (!stacked || pane !== "detail") return;
    detailRef.current?.scrollTo({ top: 0 });
  }, [pane, props.detailKey, stacked]);

  /**
   * 开始拖动分隔条，并在释放指针后清理全局监听。
   *
   * @param event 分隔条指针按下事件
   */
  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    const bounds = rootRef.current?.getBoundingClientRect();
    if (!bounds) return;
    document.body.classList.add("source-control-split-resizing");

    // 1. 指针横坐标减去容器左缘，得到左侧列表栏目标宽度
    const handlePointerMove = (moveEvent: PointerEvent) => {
      resize(moveEvent.clientX - bounds.left, bounds.width);
    };

    // 2. 指针释放或取消时清理监听和全局拖动样式
    const handlePointerUp = () => {
      document.body.classList.remove("source-control-split-resizing");
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
    window.addEventListener("pointercancel", handlePointerUp, { once: true });
  };

  /**
   * 使用方向键、Home 和 End 调整 Git 列表栏宽度。
   *
   * @param event 分隔条键盘事件
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const containerWidth = rootRef.current?.getBoundingClientRect().width ?? window.innerWidth;
    const widths: Partial<Record<string, number>> = {
      ArrowLeft: listWidth - 16,
      ArrowRight: listWidth + 16,
      Home: SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH,
      End: containerWidth - SOURCE_CONTROL_SPLIT_MIN_DETAIL_WIDTH
    };
    const nextWidth = widths[event.key];
    if (nextWidth === undefined) return;
    event.preventDefault();
    resize(nextWidth, containerWidth);
  };

  const style = { "--source-control-list-width": `${listWidth}px` } as CSSProperties;
  const maximum = Math.max(
    SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH,
    (rootRef.current?.clientWidth ?? 0) - SOURCE_CONTROL_SPLIT_MIN_DETAIL_WIDTH
  );

  if (stacked) {
    return (
      <div ref={rootRef} className={`source-control-stacked ${direction} ${props.className}`} data-pane={pane}>
        {pane === "list" ? (
          <div key="list" className="source-control-stacked-pane">
            {list}
          </div>
        ) : (
          <div key="detail" className="source-control-stacked-pane" ref={detailRef}>
            <div className="source-control-stacked-back">
              <button type="button" onClick={showList}>
                <ArrowLeft size={14} />
                {t("Back", "返回")}
              </button>
              {props.detailTitle && <span title={props.detailTitle}>{props.detailTitle}</span>}
            </div>
            {detail}
          </div>
        )}
      </div>
    );
  }

  return (
    <div
      ref={rootRef}
      className={`source-control-split ${props.className}`}
      style={style}
    >
      {props.children}
      <div
        className="source-control-split-handle"
        role="separator"
        tabIndex={0}
        aria-label={t("Resize Git list and detail panes", "调整 Git 列表与详情区域宽度")}
        aria-orientation="vertical"
        aria-valuemin={SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH}
        aria-valuemax={Math.round(maximum)}
        aria-valuenow={Math.round(listWidth)}
        onPointerDown={handlePointerDown}
        onDoubleClick={() => resize(SOURCE_CONTROL_SPLIT_DEFAULT_WIDTH, rootRef.current?.clientWidth ?? window.innerWidth)}
        onKeyDown={handleKeyDown}
      >
        <span />
      </div>
    </div>
  );
}
