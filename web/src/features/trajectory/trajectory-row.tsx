import { AlertCircle } from "lucide-react";
import { useI18n } from "../i18n/use-i18n";
import { formatDuration } from "./trajectory-format";
import { RECORD_KIND_LABELS, type TrajectoryRecord } from "./trajectory-record";

type TrajectoryRowProps = {
  record: TrajectoryRecord;
  selected: boolean;
  /** 折叠状态下该轮被隐藏的记录数；0 表示未折叠 */
  collapsedCount: number;
  onSelect: (id: string) => void;
  onToggleTurn: (turnId: string) => void;
};

/**
 * 渲染一条轨迹记录。
 *
 * 工具结果直接跟在入参之后同行显示：调用与结果分成两行时，
 * 长列表里要来回对照才能读出一次调用做了什么。
 *
 * @param props 记录数据与选中、折叠状态
 * @returns 记录行
 */
export function TrajectoryRow({
  record,
  selected,
  collapsedCount,
  onSelect,
  onToggleTurn
}: TrajectoryRowProps) {
  const { t, locale } = useI18n();
  const zh = locale.startsWith("zh");
  const kindLabel = zh ? RECORD_KIND_LABELS[record.kind].zh : RECORD_KIND_LABELS[record.kind].en;
  const resultPreview = record.detail.output?.replace(/\s+/g, " ").trim();

  return (
    <div
      className="trajectory-row"
      role="row"
      tabIndex={0}
      data-kind={record.kind}
      data-selected={selected || undefined}
      data-failed={record.failed || undefined}
      data-running={record.running || undefined}
      data-round-start={record.roundStart || undefined}
      data-nested={record.parentId ? "" : undefined}
      aria-selected={selected}
      onClick={() => onSelect(record.id)}
      onDoubleClick={() => { if (record.turnId) onToggleTurn(record.turnId); }}
      onKeyDown={(event) => {
        if (event.key !== "Enter" && event.key !== " ") return;
        event.preventDefault();
        onSelect(record.id);
      }}
    >
      <span className="trajectory-row-index">{record.index}</span>
      <span className="trajectory-row-kind">{kindLabel}</span>
      <span className="trajectory-row-body">
        {record.label && <span className="trajectory-row-label">{record.label}</span>}
        <span className="trajectory-row-summary" title={record.summary}>
          {record.summary || t("(empty)", "（空）")}
        </span>
        {resultPreview && (
          <span className="trajectory-row-result" title={resultPreview}>
            <span className="trajectory-row-arrow" aria-hidden>→</span>
            {resultPreview}
          </span>
        )}
        {record.failed && <AlertCircle size={12} className="trajectory-row-error-icon" aria-hidden />}
        {collapsedCount > 0 && (
          <button
            type="button"
            className="trajectory-row-collapsed"
            onClick={(event) => {
              event.stopPropagation();
              if (record.turnId) onToggleTurn(record.turnId);
            }}
          >
            {t(`+${collapsedCount} more`, `另 ${collapsedCount} 条`)}
          </button>
        )}
      </span>
      <span className="trajectory-row-time">
        {record.running ? t("running", "运行中") : formatDuration(record.durationMs)}
      </span>
    </div>
  );
}
