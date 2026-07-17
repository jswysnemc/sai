import { Check, ChevronDown, ListChecks, TerminalSquare, Wrench } from "lucide-react";
import { useState } from "react";
import type { ToolLifecycle } from "../run-event-reducer";
import { ToolLifecycleCard } from "../tool-lifecycle-card";
import { toolCallGroupLabel } from "./tool-call-grouping";
import "./tool-call-group.css";

/**
 * 渲染默认折叠的连续已完成工具组。
 *
 * @param props tools 为组内工具调用
 * @returns 工具组标题和可展开原始卡片
 */
export function ToolCallGroup({ tools }: { tools: ToolLifecycle[] }) {
  const [expanded, setExpanded] = useState(false);
  const todoOnly = tools.every((tool) => tool.name === "todo");
  const commandOnly = tools.every((tool) => tool.name === "run_command" || tool.name.includes("command"));
  const label = toolCallGroupLabel(tools);
  return (
    <section className={`tool-call-group${expanded ? " expanded" : ""}`}>
      <button type="button" className="tool-call-group-trigger" onClick={() => setExpanded((value) => !value)} aria-expanded={expanded}>
        <span className="tool-call-group-icon">{todoOnly ? <ListChecks size={15} /> : commandOnly ? <TerminalSquare size={15} /> : <Wrench size={15} />}</span>
        <strong>{label}</strong>
        <span className="tool-call-group-status"><Check size={14} /></span>
        <ChevronDown size={14} className={expanded ? "rotate" : ""} aria-hidden />
      </button>
      {expanded && (
        <div className="tool-call-group-items">
          {tools.map((tool) => <ToolLifecycleCard key={tool.id} tool={tool} />)}
        </div>
      )}
    </section>
  );
}
