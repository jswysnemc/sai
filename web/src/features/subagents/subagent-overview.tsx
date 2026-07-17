import { useQuery } from "@tanstack/react-query";
import { Bot, RefreshCw } from "lucide-react";
import { api } from "../../api/client";
import type { Subagent } from "../../api/contracts";
import { SubagentCard } from "./subagent-card";

type SubagentOverviewProps = {
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCancel: (id: string) => void;
};

/**
 * 渲染子智能体概览列表,展示运行计数、实时进度并支持选中查看详情。
 *
 * @param props 选中项、选择与取消回调
 * @returns 子智能体概览
 */
export function SubagentOverview({ selectedId, onSelect, onCancel }: SubagentOverviewProps) {
  const query = useQuery({ queryKey: ["subagents"], queryFn: api.subagents.list, refetchInterval: 2000 });
  const subagents = query.data ?? [];
  const running = subagents.filter((subagent: Subagent) => subagent.status === "running").length;
  return (
    <div className="subagent-overview">
      <header className="subagent-overview-head">
        <div className="subagent-overview-title">
          <Bot size={15} />
          <strong>子智能体</strong>
          <span className="subagent-overview-count">{running > 0 ? `${running} 运行中 · 共 ${subagents.length}` : `${subagents.length} 个`}</span>
        </div>
        <button type="button" onClick={() => void query.refetch()} aria-label="刷新子智能体"><RefreshCw size={13} /></button>
      </header>
      <div className="subagent-overview-list">
        {subagents.map((subagent: Subagent) => (
          <SubagentCard
            key={subagent.id}
            subagent={subagent}
            active={subagent.id === selectedId}
            onSelect={() => onSelect(subagent.id)}
            onCancel={() => onCancel(subagent.id)}
          />
        ))}
        {!query.isLoading && subagents.length === 0 && (
          <div className="subagent-overview-empty">
            <Bot size={26} />
            <p>还没有子智能体</p>
            <span>主对话调用 task 工具后,子智能体会在这里实时显示进度</span>
          </div>
        )}
      </div>
      {query.error && <div className="pane-error">{query.error.message}</div>}
    </div>
  );
}
