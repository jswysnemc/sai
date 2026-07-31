import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { useI18n } from "../../i18n/use-i18n";
import {
  SOURCE_CONTROL_SPLIT_DEFAULT_WIDTH,
  SOURCE_CONTROL_SPLIT_MIN_DETAIL_WIDTH,
  SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH,
  shouldStackSourceControlSplit,
  useSourceControlSplitState
} from "./source-control-split-state";
import "./source-control-split-view.css";

type SourceControlSplitViewProps = {
  className: string;
  children: ReactNode;
};

/**
 * 渲染可调整左右宽度的 Git 双栏视图。
 *
 * @param props 外层类名和左右两侧内容
 * @returns 带可访问分隔条的 Git 双栏布局
 */
export function SourceControlSplitView(props: SourceControlSplitViewProps) {
  const { t } = useI18n();
  const rootRef = useRef<HTMLDivElement>(null);
  const [stacked, setStacked] = useState(false);
  const { listWidth, resize, constrain } = useSourceControlSplitState();

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

  /**
   * 开始拖动分隔条，并在释放指针后清理全局监听。
   *
   * @param event 分隔条指针按下事件
   */
  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (stacked) return;
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

  return (
    <div
      ref={rootRef}
      className={`source-control-split${stacked ? " is-stacked" : ""} ${props.className}`}
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
