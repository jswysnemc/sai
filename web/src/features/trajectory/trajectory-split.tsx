import {
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode
} from "react";
import { useI18n } from "../i18n/use-i18n";
import "./trajectory-split.css";

type TrajectorySplitProps = {
  left: ReactNode;
  right: ReactNode;
  leftLabel: string;
  rightLabel: string;
};

const LEFT_DEFAULT = 62;
const LEFT_MIN = 32;
const RIGHT_MIN = 24;

/**
 * 渲染总览与详情两栏，中间分隔条可拖拽调宽。
 *
 * @param props 左右内容和栏目标题
 * @returns 可调整宽度的双栏布局
 */
export function TrajectorySplit({ left, right, leftLabel, rightLabel }: TrajectorySplitProps) {
  const { t } = useI18n();
  const rootRef = useRef<HTMLDivElement>(null);
  const [leftPercent, setLeftPercent] = useState(LEFT_DEFAULT);

  /**
   * 把目标宽度限制在两侧都能放下的范围内。
   *
   * @param percent 左侧百分比
   * @returns 夹紧后的百分比
   */
  const clamp = (percent: number): number => Math.min(100 - RIGHT_MIN, Math.max(LEFT_MIN, percent));

  /**
   * 开始拖动分隔条，并在指针释放后清理全局监听。
   *
   * @param event 分隔条指针按下事件
   * @returns 无
   */
  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>): void => {
    event.preventDefault();
    const bounds = rootRef.current?.getBoundingClientRect();
    if (!bounds) return;
    document.body.classList.add("trajectory-split-resizing");

    const handlePointerMove = (moveEvent: PointerEvent) => {
      setLeftPercent(clamp(((moveEvent.clientX - bounds.left) / bounds.width) * 100));
    };
    const handlePointerUp = () => {
      document.body.classList.remove("trajectory-split-resizing");
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
    };
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
    window.addEventListener("pointercancel", handlePointerUp, { once: true });
  };

  /**
   * 使用方向键微调左右栏宽度。
   *
   * @param event 分隔条键盘事件
   * @returns 无
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    const steps: Partial<Record<string, number>> = {
      ArrowLeft: leftPercent - 2,
      ArrowRight: leftPercent + 2,
      Home: LEFT_MIN,
      End: 100 - RIGHT_MIN
    };
    const next = steps[event.key];
    if (next === undefined) return;
    event.preventDefault();
    setLeftPercent(clamp(next));
  };

  const style = { "--trajectory-split-left": `${leftPercent}%` } as CSSProperties;

  return (
    <div ref={rootRef} className="trajectory-split" style={style}>
      <section className="trajectory-split-pane" aria-label={leftLabel}>
        {left}
      </section>
      <div
        className="trajectory-split-handle"
        role="separator"
        aria-orientation="vertical"
        aria-label={t("Resize overview and details", "调整总览与详情栏宽度")}
        aria-valuemin={LEFT_MIN}
        aria-valuemax={100 - RIGHT_MIN}
        aria-valuenow={Math.round(leftPercent)}
        tabIndex={0}
        onPointerDown={handlePointerDown}
        onKeyDown={handleKeyDown}
      >
        <span />
      </div>
      <section className="trajectory-split-pane" aria-label={rightLabel}>
        {right}
      </section>
    </div>
  );
}
