import { MarkdownRenderer } from "../chat/markdown-renderer";

/**
 * 【计划模式】【完整审阅】展示当前审批的固定计划快照，正文可独立滚动且不截断。
 * @param props 当前审批计划正文
 * @returns 响应式 Markdown 审阅区域
 */
export function PlanReviewContent({ plan }: { plan: string }) {
  return (
    <div aria-label="Plan review" className="mx-2 my-2 max-h-[55vh] min-w-0 overflow-auto rounded-md border border-current/10 p-3 text-[0.875rem] sm:mx-3 sm:p-4 sm:text-[0.9375rem] [&_pre]:max-w-full [&_pre]:overflow-x-auto">
      <MarkdownRenderer source={plan} />
    </div>
  );
}
