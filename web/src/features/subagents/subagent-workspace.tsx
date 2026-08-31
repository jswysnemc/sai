import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { SubagentDetailView } from "./subagent-detail-view";
import { FOCUS_SUBAGENT_EVENT, focusIdFromEvent, takePendingSubagentFocus } from "./subagent-focus";
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
  // 概览点击条目时面板可能尚未挂载，首次渲染先认领待聚焦项
  const [selectedId, setSelectedId] = useState<string | null>(() => takePendingSubagentFocus());
  const query = useQuery({ queryKey: ["subagents"], queryFn: api.subagents.list, refetchInterval: 2000 });
  const cancel = useMutation({
    mutationFn: api.subagents.cancel,
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["subagents"] })
  });
  const selected = query.data?.find((subagent) => subagent.id === selectedId) ?? null;

  // 面板已经打开时，概览的后续点击靠事件切换详情
  useEffect(() => {
    const handleFocus = (event: Event) => {
      const id = focusIdFromEvent(event);
      if (id) setSelectedId(id);
    };
    window.addEventListener(FOCUS_SUBAGENT_EVENT, handleFocus);
    return () => window.removeEventListener(FOCUS_SUBAGENT_EVENT, handleFocus);
  }, []);

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
      {cancel.error && <div className="pane-error">{cancel.error.message}</div>}
    </div>
  );
}
