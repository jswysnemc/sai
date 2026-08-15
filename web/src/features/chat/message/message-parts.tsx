import type { LiveMessagePart } from "../run-event-reducer";
import { MarkdownRenderer } from "../markdown-renderer";
import { ReasoningBlock } from "../reasoning-block";
import { ToolLifecycleCard } from "../tool-lifecycle-card";
import { ContextCompactionPart } from "./context-compaction-part";
import { PermissionRequestCard } from "../../permission/permission-request-card";
import { QuestionRequestCard } from "../../question/question-request-card";
import { SshSecretCard } from "../../ssh/ssh-secret-card";
import { AutomaticInputPart } from "./automatic-input-part";
import { EngineReadyPart } from "./engine-ready-part";

/**
 * 按消息部件顺序渲染思考、正文和工具调用。
 *
 * 部件一律平铺：把多次调用折进一行组头后，做过什么只剩一句分类计数，
 * 要看具体调用得先展开，而思考在折叠态里根本不出现。
 *
 * @param props 有序消息部件及实时运行状态
 * @returns 嵌入同一助手消息中的部件列表
 */
export function MessageParts({ parts, live }: { parts: LiveMessagePart[]; live?: boolean }) {
  return (
    <div className="message-parts">
      {parts.map((part) => {
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
      })}
    </div>
  );
}
