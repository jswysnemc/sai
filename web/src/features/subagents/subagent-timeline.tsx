import { Brain, CircleCheck, CircleX, Loader2, Wrench } from "lucide-react";
import { useState } from "react";
import type { SubagentTimelineEntry } from "../../api/contracts";
import { MarkdownRenderer } from "../chat/markdown-renderer";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染子智能体执行时间线:推理片段、轮间正文与工具调用按发生顺序排列。
 *
 * 正文复用主对话的 MarkdownRenderer,运行中随轮询增量出现,形成流式观感。
 *
 * @param props 时间线条目与运行状态
 * @returns 时间线列表,无条目时返回 null
 */
export function SubagentTimeline({ entries, running }: { entries: SubagentTimelineEntry[]; running: boolean }) {
  if (entries.length === 0) return null;
  return (
    <ol className="subagent-timeline">
      {entries.map((entry, index) => (
        <li key={index} className={`subagent-timeline-item is-${entry.kind}`}>
          {entry.kind === "tool" && <ToolEntry entry={entry} running={running} />}
          {entry.kind === "reasoning" && <ReasoningEntry text={entry.text} />}
          {entry.kind === "text" && <div className="subagent-timeline-text"><MarkdownRenderer source={entry.text} /></div>}
        </li>
      ))}
    </ol>
  );
}

/**
 * 渲染单条工具调用条目,可展开查看参数与输出预览。
 *
 * @param props 工具条目与整体运行状态
 * @returns 工具调用行
 */
function ToolEntry({ entry, running }: { entry: Extract<SubagentTimelineEntry, { kind: "tool" }>; running: boolean }) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(false);
  const pending = entry.ok == null;
  const detail = [entry.args_preview && t(`Arguments ${entry.args_preview}`, `参数 ${entry.args_preview}`), entry.output_preview && t(`Output ${entry.output_preview}`, `输出 ${entry.output_preview}`)].filter(Boolean).join("\n");
  return (
    <div className="subagent-timeline-tool">
      <button type="button" onClick={() => detail && setExpanded((value) => !value)} aria-expanded={detail ? expanded : undefined}>
        <span className={`subagent-timeline-tool-state${pending ? " pending" : entry.ok ? " ok" : " failed"}`}>
          {pending ? (running ? <Loader2 size={13} className="spin" /> : <Wrench size={13} />) : entry.ok ? <CircleCheck size={13} /> : <CircleX size={13} />}
        </span>
        <span className="subagent-timeline-tool-step">#{entry.step}</span>
        <span className="subagent-timeline-tool-name">{entry.name}</span>
        {pending && running && <span className="subagent-timeline-tool-hint">{t("Running", "运行中")}</span>}
      </button>
      {expanded && detail && <pre className="subagent-timeline-tool-detail">{detail}</pre>}
    </div>
  );
}

/**
 * 渲染可折叠的推理片段,默认收起只显示首行摘要。
 *
 * @param props 推理文本
 * @returns 推理条目
 */
function ReasoningEntry({ text }: { text: string }) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(false);
  const firstLine = text.split("\n").find((line) => line.trim()) ?? "";
  return (
    <div className={`subagent-timeline-reasoning${expanded ? " expanded" : ""}`}>
      <button type="button" onClick={() => setExpanded((value) => !value)} aria-expanded={expanded}>
        <Brain size={12} />
        <span>{expanded ? t("Reasoning", "思考过程") : firstLine || t("Reasoning", "思考过程")}</span>
      </button>
      {expanded && <p>{text}</p>}
    </div>
  );
}
