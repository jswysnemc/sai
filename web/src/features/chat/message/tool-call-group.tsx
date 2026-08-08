import { ChevronDown, FilePenLine, ListChecks, Search, ShieldCheck, TerminalSquare, Wrench } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../../api/client";
import { Button } from "../../../shared/ui/button/button";
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
  const { locale, t } = useI18n();
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
  // 组内经过权限审核的项数：折叠态也要能看出这批操作动过权限
  const auditedCount = tools.filter((tool) => tool.permission).length;
  const icon = todoOnly
    ? <ListChecks size={14} />
    : exploreOnly
      ? <Search size={14} />
      : commandOnly
        ? <TerminalSquare size={14} />
        : editOnly
          ? <FilePenLine size={14} />
          : <Wrench size={14} />;
  return (
    <section className={`tool-call-group${expanded ? " expanded" : ""}`}>
      <Button className="tool-call-group-trigger" onClick={() => setExpanded((value) => !value)} aria-expanded={expanded}>
        <span className="tool-call-group-icon">{icon}</span>
        <strong title={label}>{label}</strong>
        {auditedCount > 0 && (
          <span
            className="tool-call-group-audited"
            title={t(`${auditedCount} of them required a permission decision`, `其中 ${auditedCount} 项经过权限审核`)}
          >
            <ShieldCheck size={11} aria-hidden />
            {auditedCount}
          </span>
        )}
        <ChevronDown size={14} className={expanded ? "rotate" : ""} aria-hidden />
      </Button>
      {expanded && (
        <div className="tool-call-group-items">
          {tools.map((tool) => <ToolLifecycleCard key={tool.id} tool={tool} />)}
        </div>
      )}
    </section>
  );
}
