import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../api/client";
import { SubagentDetailView } from "./subagent-detail-view";
import { SubagentOverview } from "./subagent-overview";
import "./subagents.css";

/**
 * 渲染子智能体工作区:概览与详情主从切换。
 *
 * 作为编程页与文件、Git、终端平级的独立视图,选中概览中的子智能体后
 * 进入详情,详情区复用 Markdown 渲染展示结果输出。
 *
 * @returns 子智能体工作区
 */
export function SubagentWorkspace() {
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const query = useQuery({ queryKey: ["subagents"], queryFn: api.subagents.list, refetchInterval: 2000 });
  const cancel = useMutation({
    mutationFn: api.subagents.cancel,
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["subagents"] })
  });
  const selected = query.data?.find((subagent) => subagent.id === selectedId) ?? null;

  return (
    <div className="subagent-workspace">
      {selected ? (
        <SubagentDetailView
          subagent={selected}
          onBack={() => setSelectedId(null)}
          onCancel={(id) => cancel.mutate(id)}
        />
      ) : (
        <SubagentOverview
          selectedId={selectedId}
          onSelect={setSelectedId}
          onCancel={(id) => cancel.mutate(id)}
        />
      )}
    </div>
  );
}
