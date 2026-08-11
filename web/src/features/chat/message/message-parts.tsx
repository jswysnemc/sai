import type { LiveMessagePart } from "../run-event-reducer";
import { MarkdownRenderer } from "../markdown-renderer";
import { ReasoningBlock } from "../reasoning-block";
import { ToolLifecycleCard } from "../tool-lifecycle-card";
import { ToolCallGroup } from "./tool-call-group";
import { StepGroupCard } from "./step-group";
import { groupIntoSteps } from "./step-grouping";
import { ContextCompactionPart } from "./context-compaction-part";
import { PermissionRequestCard } from "../../permission/permission-request-card";
import { QuestionRequestCard } from "../../question/question-request-card";
import { AutomaticInputPart } from "./automatic-input-part";
import { EngineReadyPart } from "./engine-ready-part";

/**
 * 按消息部件顺序渲染思考、正文和工具调用。
 *
 * @param props 有序消息部件及实时运行状态
 * @returns 嵌入同一助手消息中的部件列表
 */
export function MessageParts({ parts, live }: { parts: LiveMessagePart[]; live?: boolean }) {
  // 先按「思考 + 工具」步骤聚合，未成步骤的部件回落到相邻工具分组
  const groupedParts = groupIntoSteps(parts);
  return (
    <div className="message-parts">
      {groupedParts.map((item) => {
        if (item.type === "step") {
          return (
            <StepGroupCard
              key={item.id}
              id={item.id}
              reasoning={item.reasoning}
              tools={item.tools}
            />
          );
        }
        if (item.type === "tool-group") return <ToolCallGroup key={item.id} tools={item.tools} live={Boolean(live)} />;
        const part = item.part;
        if (part.type === "reasoning") {
          return <ReasoningBlock key={item.id} source={part.source} live={live && !part.endedAt} startedAt={part.startedAt} endedAt={part.endedAt} />;
        }
        if (part.type === "tool") return <ToolLifecycleCard key={item.id} tool={part.tool} />;
        if (part.type === "permission") return <PermissionRequestCard key={item.id} request={part.request} decision={part.decision} active={Boolean(live)} />;
        if (part.type === "question") return <QuestionRequestCard key={item.id} pending={part.pending} response={part.response} active={Boolean(live)} />;
        if (part.type === "compaction") return <ContextCompactionPart key={item.id} part={part} />;
        if (part.type === "automatic_input") return <AutomaticInputPart key={item.id} content={part.source} />;
        if (part.type === "engine_ready") return <EngineReadyPart key={item.id} engine={part.engine} version={part.version} />;
        return <MarkdownRenderer key={item.id} source={part.source} streaming={Boolean(live)} />;
      })}
    </div>
  );
}
