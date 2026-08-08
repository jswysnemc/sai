import { ChevronDown, Footprints, ShieldCheck } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../../api/client";
import { Button } from "../../../shared/ui/button/button";
import { ReasoningBlock, reasoningDuration } from "../reasoning-block";
import type { LiveMessagePart, ToolLifecycle } from "../run-event-reducer";
import { ToolLifecycleCard } from "../tool-lifecycle-card";
import { toolCallGroupLabel } from "./tool-call-grouping";
import { groupHasExpandedTool, usePersistedExpand } from "./tool-expand-state";
import { useI18n } from "../../i18n/use-i18n";
import "./step-group.css";

type StepGroupCardProps = {
  /** 步骤标识，用于记忆展开状态 */
  id: string;
  /** 该步骤的思考部件 */
  reasoning: Extract<LiveMessagePart, { type: "reasoning" }>;
  /** 思考之后执行的工具 */
  tools: ToolLifecycle[];
};

/**
 * 渲染默认折叠的「思考 + 工具调用」步骤组。
 *
 * 折叠态用一行同时表达"想了多久"与"做了什么"，展开后原样渲染内部的思考块
 * 与工具卡，不改变它们各自的交互。
 *
 * @param props 步骤标识、思考部件与工具列表
 * @returns 步骤组
 */
export function StepGroupCard({ id, reasoning, tools }: StepGroupCardProps) {
  const { locale, t } = useI18n();
  const workspaces = useQuery({
    queryKey: ["workspaces"],
    queryFn: api.workspaces.list,
    staleTime: 30_000
  });
  const workspacePath =
    workspaces.data?.workspaces.find((item) => item.id === workspaces.data?.active_id)?.path ?? "";
  // 组内任一工具已被用户展开时，整个步骤保持展开
  const [expanded, setExpanded] = usePersistedExpand(
    id,
    groupHasExpandedTool(tools.map((tool) => tool.id))
  );
  // 步骤已结束，时长取固定值即可，无需随时钟刷新
  const duration = reasoningDuration(reasoning.startedAt, reasoning.endedAt, 0, locale);
  const actions = toolCallGroupLabel(tools, locale, workspacePath);
  const auditedCount = tools.filter((tool) => tool.permission).length;
  const label = duration
    ? t(`Thought ${duration} · ${actions}`, `思考 ${duration} · ${actions}`)
    : actions;

  return (
    <section className={`step-group${expanded ? " expanded" : ""}`}>
      <Button
        className="step-group-trigger"
        onClick={() => setExpanded((value) => !value)}
        aria-expanded={expanded}
      >
        <span className="step-group-icon">
          <Footprints size={14} />
        </span>
        <strong title={label}>{label}</strong>
        {auditedCount > 0 && (
          <span
            className="step-group-audited"
            title={t(
              `${auditedCount} of them required a permission decision`,
              `其中 ${auditedCount} 项经过权限审核`
            )}
          >
            <ShieldCheck size={11} aria-hidden />
            {auditedCount}
          </span>
        )}
        <ChevronDown size={14} className={expanded ? "rotate" : ""} aria-hidden />
      </Button>
      {expanded && (
        <div className="step-group-items">
          <ReasoningBlock
            source={reasoning.source}
            startedAt={reasoning.startedAt}
            endedAt={reasoning.endedAt}
          />
          {tools.map((tool) => (
            <ToolLifecycleCard key={tool.id} tool={tool} />
          ))}
        </div>
      )}
    </section>
  );
}
