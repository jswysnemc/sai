import { Archive, Loader2 } from "lucide-react";
import type { LiveMessagePart } from "../run-event-reducer";
import { MarkdownRenderer } from "../markdown-renderer";
import { ErrorDetailToggle } from "./error-detail-toggle";
import { useI18n } from "../../i18n/use-i18n";

type CompactionPart = Extract<LiveMessagePart, { type: "compaction" }>;

/**
 * 渲染运行期间的上下文压缩状态，并在应用成功后展示压缩摘要。
 *
 * @param props 压缩状态部件
 * @returns 状态行；成功应用时附带分割线与摘要内容
 */
export function ContextCompactionPart({ part }: { part: CompactionPart }) {
  const { t } = useI18n();
  const running = part.status === "running";
  const text = running
    ? part.summary
      ? t(`Generating a compaction summary for ${part.turnCount} turns`, `正在生成 ${part.turnCount} 轮会话的压缩摘要`)
      : t(`Compacting ${part.turnCount} old turns`, `正在压缩 ${part.turnCount} 轮旧上下文`)
    : part.applied
      ? t(`Compacted ${part.turnCount} old turns`, `已压缩 ${part.turnCount} 轮旧上下文`)
      : part.turnCount === 0
        ? t("No old conversation turns can be compacted", "没有可压缩的旧会话轮次")
        : part.error?.message ?? t("Context compaction was not applied", "本次上下文压缩未应用");
  const summary = part.summary?.trim() || null;
  const dividerLabel = running
    ? t("Compacting earlier conversation", "正在压缩此前会话")
    : part.applied
      ? t("Earlier conversation compacted", "此前会话已压缩")
      : t("Earlier conversation compaction incomplete", "此前会话压缩未完成");

  return (
    <div className={`context-compaction-block${running ? " running" : ""}`}>
      <div className="context-compaction-part">
        {running ? <Loader2 size={14} className="spin" /> : <Archive size={14} />}
        <span>{text}{part.model ? ` · ${part.model}` : ""}</span>
      </div>
      <div className="context-compaction-divider" role="separator" aria-label={dividerLabel}>
        <span className="context-compaction-divider-line" />
        <span className="context-compaction-divider-label">{dividerLabel}</span>
        <span className="context-compaction-divider-line" />
      </div>
      {summary && (
        <div className="context-compaction-summary">
          <MarkdownRenderer source={summary} />
        </div>
      )}
      {part.error && <ErrorDetailToggle detail={part.error.detail} />}
    </div>
  );
}
