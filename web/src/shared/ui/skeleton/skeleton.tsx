import "./skeleton.css";

type SkeletonProps = {
  /** 骨架条宽度，任意 CSS 长度，默认撑满 */
  width?: string;
  /** 骨架条高度，任意 CSS 长度，默认一行正文高 */
  height?: string;
  /** 附加类名 */
  className?: string;
};

/**
 * 单条骨架占位。
 *
 * @param props 尺寸与类名
 * @returns 骨架条元素
 */
export function Skeleton({ width, height = "0.875rem", className }: SkeletonProps) {
  return (
    <span
      aria-hidden="true"
      className={className ? `ui-skeleton ${className}` : "ui-skeleton"}
      style={{ width: width ?? "100%", height }}
    />
  );
}

type SkeletonTextProps = {
  /** 骨架行数，默认 3 */
  lines?: number;
  /** 无障碍朗读的加载说明 */
  label: string;
};

/**
 * 段落骨架，用于替代成段正文的加载空白。
 *
 * @param props 行数与无障碍说明
 * @returns 段落骨架元素
 */
export function SkeletonText({ lines = 3, label }: SkeletonTextProps) {
  return (
    <div aria-label={label} className="ui-skeleton-group ui-skeleton-text" role="status">
      {Array.from({ length: lines }, (_, index) => (
        <Skeleton key={index} />
      ))}
    </div>
  );
}

type SkeletonListProps = {
  /** 骨架条目数，默认 5 */
  items?: number;
  /** 无障碍朗读的加载说明 */
  label: string;
};

/**
 * 列表骨架，每项由标题行与摘要行组成。
 *
 * @param props 条目数与无障碍说明
 * @returns 列表骨架元素
 */
export function SkeletonList({ items = 5, label }: SkeletonListProps) {
  return (
    <div aria-label={label} className="ui-skeleton-list" role="status">
      {Array.from({ length: items }, (_, index) => (
        <div className="ui-skeleton-list-item" key={index}>
          <Skeleton height="0.75rem" />
          <Skeleton height="0.625rem" />
        </div>
      ))}
    </div>
  );
}
