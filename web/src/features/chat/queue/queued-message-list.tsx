import { useState } from "react";
import type { LiveRunState } from "../run-event-reducer";
import { QueuedMessageRow } from "./queued-message-row";
import { useI18n } from "../../i18n/use-i18n";
import "./queued-message-list.css";

export type QueuedMessageListProps = {
  runs: LiveRunState[];
  onUpdate: (runId: string, input: string) => Promise<void>;
  onMove: (runId: string, position: number) => Promise<void>;
  onRemove: (runId: string) => Promise<void>;
  onError: (error: unknown) => void;
};

/**
 * 将指定排队运行移动到目标位置。
 *
 * @param runs 当前排队运行
 * @param runId 待移动运行标识
 * @param position 从零开始的目标位置
 * @returns 调整顺序后的新数组
 */
export function reorderQueuedRuns(
  runs: LiveRunState[],
  runId: string,
  position: number
): LiveRunState[] {
  const current = runs.findIndex((run) => run.runId === runId);
  if (current < 0) return runs;
  const next = [...runs];
  const [selected] = next.splice(current, 1);
  next.splice(Math.max(0, Math.min(position, next.length)), 0, selected);
  return next;
}

/**
 * 渲染当前会话的连续消息队列面板。
 *
 * @param props 排队运行和编辑、排序、删除回调
 * @returns 紧凑队列列表；队列为空时不渲染
 */
export function QueuedMessageList({ runs, onUpdate, onMove, onRemove, onError }: QueuedMessageListProps) {
  const { t } = useI18n();
  const [draggedRunId, setDraggedRunId] = useState<string | null>(null);
  if (runs.length === 0) return null;

  /**
   * 将拖动中的运行移动到当前行位置。
   *
   * @param position 目标行位置
   * @returns 排序请求完成后的 Promise
   */
  const dropAt = async (position: number) => {
    if (!draggedRunId) return;
    setDraggedRunId(null);
    try {
      await onMove(draggedRunId, position);
    } catch (error) {
      onError(error);
    }
  };

  return (
    <section className="queued-message-list" aria-label={t("Message queue", "消息队列")}>
      <header className="queued-message-list-head">
        <span>{t("Message queue", "消息队列")}</span>
        <small>{t(`${runs.length} waiting`, `${runs.length} 条待发送`)}</small>
      </header>
      {runs.map((run, index) => (
        <QueuedMessageRow
          key={run.runId}
          run={run}
          position={index}
          total={runs.length}
          dragging={draggedRunId === run.runId}
          onDragStart={() => setDraggedRunId(run.runId)}
          onDragEnd={() => setDraggedRunId(null)}
          onDrop={() => void dropAt(index)}
          onUpdate={onUpdate}
          onMove={onMove}
          onRemove={onRemove}
          onError={onError}
        />
      ))}
    </section>
  );
}
