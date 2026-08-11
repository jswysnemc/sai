import { AnimatedCount } from "../animated-count";

type ToolDiffBadgeProps = {
  /** 新增行数 */
  added: number;
  /** 删除行数 */
  removed: number;
  /** 是否启用滚动数字组件（历史静态展示可关） */
  animate?: boolean;
  /** 是否仍在流式逼近目标值；结束后钉死最终值 */
  active?: boolean;
};

/**
 * 渲染 diff 增删行数徽章。
 *
 * 用等宽数字对齐，让同一组文件的增删量竖向可比；
 * 增删各自着色，不靠正负号区分。
 *
 * @param props 增删行数
 * @returns 增删徽章；两者皆为 0 时不渲染
 */
export function ToolDiffBadge({
  added,
  removed,
  animate = false,
  active = false
}: ToolDiffBadgeProps) {
  if (added <= 0 && removed <= 0) return null;
  return (
    <span className="inline-flex shrink-0 items-center gap-1 whitespace-nowrap font-mono leading-none tabular-nums">
      {added > 0 ? (
        <span className="text-diff-added">
          +{animate ? <AnimatedCount value={added} active={active} /> : added}
        </span>
      ) : null}
      {removed > 0 ? (
        <span className="text-diff-removed">
          -{animate ? <AnimatedCount value={removed} active={active} /> : removed}
        </span>
      ) : null}
    </span>
  );
}
