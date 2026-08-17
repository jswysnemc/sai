import type { LiveMessagePart } from "../run-event-reducer";
import { MarkdownRenderer } from "../markdown-renderer";
import { ToolLifecycleCard } from "../tool-lifecycle-card";
import { ContextCompactionPart } from "./context-compaction-part";
import { PermissionRequestCard } from "../../permission/permission-request-card";
import { QuestionRequestCard } from "../../question/question-request-card";
import { SshSecretCard } from "../../ssh/ssh-secret-card";
import { AutomaticInputPart } from "./automatic-input-part";
import { EngineReadyPart } from "./engine-ready-part";
import { ActivityPreamble } from "./activity-preamble";
import { groupActivityParts } from "./group-activity-parts";
import { ReasoningBlock } from "../reasoning-block";

/**
 * 按消息部件顺序渲染思考、正文和工具调用。
 *
 * 正文前的连续思考与工具收成一组，完成后默认折叠；并行工具按开始/结束事件排队轮播。
 *
 * @param props 有序消息部件及实时运行状态
 * @returns 嵌入同一助手消息中的部件列表
 */
export function MessageParts({ parts, live }: { parts: LiveMessagePart[]; live?: boolean }) {
  return (
    <div className="message-parts">
      {groupActivityParts(parts).map((segment) => {
        if (segment.type === "preamble") {
          return (
            <ActivityPreamble
              key={segment.id}
              id={segment.id}
              items={segment.items}
              followedByText={segment.followedByText}
              live={live}
            />
          );
        }
        return renderStandalonePart(segment.part, live);
      })}
    </div>
  );
}

/**
 * 渲染无法编入工作组的单个部件。
 *
 * @param part 消息部件
 * @param live 是否为实时运行
 * @returns 部件节点
 */
function renderStandalonePart(part: LiveMessagePart, live?: boolean) {
  if (part.type === "reasoning") {
    return <ReasoningBlock key={part.id} source={part.source} live={live && !part.endedAt} startedAt={part.startedAt} endedAt={part.endedAt} />;
  }
  if (part.type === "tool") return <ToolLifecycleCard key={part.id} tool={part.tool} />;
  if (part.type === "permission") return <PermissionRequestCard key={part.id} request={part.request} decision={part.decision} active={Boolean(live)} />;
  if (part.type === "question") return <QuestionRequestCard key={part.id} pending={part.pending} response={part.response} active={Boolean(live)} />;
  if (part.type === "ssh_secret") return <SshSecretCard key={part.id} request={part.request} resolved={part.resolved} active={Boolean(live)} />;
  if (part.type === "compaction") return <ContextCompactionPart key={part.id} part={part} />;
  if (part.type === "automatic_input") return <AutomaticInputPart key={part.id} content={part.source} />;
  if (part.type === "engine_ready") return <EngineReadyPart key={part.id} engine={part.engine} version={part.version} />;
  return <MarkdownRenderer key={part.id} source={part.source} streaming={Boolean(live)} />;
}
