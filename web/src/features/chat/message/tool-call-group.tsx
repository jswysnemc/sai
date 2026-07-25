import { Check, ChevronDown, FilePenLine, ListChecks, Search, TerminalSquare, Wrench } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../../api/client";
import { groupHasExpandedTool, usePersistedExpand } from "./tool-expand-state";
import type { ToolLifecycle } from "../run-event-reducer";
import { ToolLifecycleCard } from "../tool-lifecycle-card";
import {
  isCommandTool,
  isEditTool,
  isExploreTool,
  toolCallGroupLabel
} from "./tool-call-grouping";
import { useI18n } from "../../i18n/use-i18n";
import "./tool-call-group.css";

/**
 * 渲染默认折叠的连续已完成工具组。
 *
 * @param props tools 为组内工具调用
 * @returns 工具组标题和可展开原始卡片
 */
export function ToolCallGroup({ tools }: { tools: ToolLifecycle[] }) {
  const { locale } = useI18n();
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list, staleTime: 30_000 });
  const workspacePath = workspaces.data?.workspaces.find((item) => item.id === workspaces.data?.active_id)?.path ?? "";
  // 组 id 用首项稳定；若用户已展开组内任一工具则保持展开
  const groupId = tools[0]?.id ? `tool-group-${tools[0].id}` : "tool-group";
  const [expanded, setExpanded] = usePersistedExpand(
    groupId,
    groupHasExpandedTool(tools.map((tool) => tool.id))
  );
  const todoOnly = tools.every((tool) => tool.name === "todo");
  const exploreOnly = tools.every(isExploreTool);
  const commandOnly = tools.every(isCommandTool);
  const editOnly = tools.every(isEditTool);
  const label = toolCallGroupLabel(tools, locale, workspacePath);
  const icon = todoOnly
    ? <ListChecks size={15} />
    : exploreOnly
      ? <Search size={15} />
      : commandOnly
        ? <TerminalSquare size={15} />
        : editOnly
          ? <FilePenLine size={15} />
          : <Wrench size={15} />;
  return (
    <section className={`tool-call-group${expanded ? " expanded" : ""}`}>
      <button type="button" className="tool-call-group-trigger" onClick={() => setExpanded((value) => !value)} aria-expanded={expanded}>
        <span className="tool-call-group-icon">{icon}</span>
        <strong title={label}>{label}</strong>
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
