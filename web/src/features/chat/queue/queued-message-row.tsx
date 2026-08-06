import { ArrowUpToLine, Check, GripVertical, Pencil, Trash2, X } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Button } from "../../../shared/ui/button/button";
import { TextArea } from "../../../shared/ui/form/text-area";
import type { LiveRunState } from "../run-event-reducer";
import { useI18n } from "../../i18n/use-i18n";
import { UserMessageBubble } from "../message/user-message-bubble";

type QueueAction = "move" | "update" | "remove";

type QueuedMessageRowProps = {
  run: LiveRunState;
  position: number;
  total: number;
  dragging: boolean;
  onDragStart: () => void;
  onDragEnd: () => void;
  onDrop: () => void;
  onUpdate: (runId: string, input: string) => Promise<void>;
  onMove: (runId: string, position: number) => Promise<void>;
  onRemove: (runId: string) => Promise<void>;
  onError: (error: unknown) => void;
};

/**
 * 渲染一条可编辑、可排序的排队消息。
 *
 * @param props 排队运行、当前位置和操作回调
 * @returns 队列消息行
 */
export function QueuedMessageRow(props: QueuedMessageRowProps) {
  const { t } = useI18n();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(props.run.userInput);
  const [busy, setBusy] = useState<QueueAction | null>(null);
  const editorRef = useRef<HTMLTextAreaElement>(null);
  const runId = props.run.runId;

  useEffect(() => {
    if (!editing) setDraft(props.run.userInput);
  }, [editing, props.run.userInput]);

  useEffect(() => {
    if (editing) editorRef.current?.focus();
  }, [editing]);

  /**
   * 执行队列异步操作，并统一处理忙碌状态和错误。
   *
   * @param action 操作种类
   * @param operation 实际异步操作
   * @returns 操作完成后的 Promise
   */
  const perform = async (action: QueueAction, operation: () => Promise<void>) => {
    if (busy || !runId) return;
    setBusy(action);
    try {
      await operation();
    } catch (error) {
      props.onError(error);
      throw error;
    } finally {
      setBusy(null);
    }
  };

  /**
   * 保存编辑后的消息正文。
   *
   * @returns 保存完成后的 Promise
   */
  const save = async () => {
    if (!runId || (!draft.trim() && props.run.imageUrls.length === 0)) return;
    try {
      await perform("update", () => props.onUpdate(runId, draft));
      setEditing(false);
    } catch {
      // 错误已交由聊天页统一展示，保留编辑状态供用户修正
    }
  };

  /**
   * 处理编辑框保存和取消快捷键。
   *
   * @param event 编辑框键盘事件
   * @returns 无
   */
  const handleEditorKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      setDraft(props.run.userInput);
      setEditing(false);
      return;
    }
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void save();
    }
  };

  /**
   * 处理拖动手柄的键盘排序。
   *
   * @param event 手柄键盘事件
   * @returns 无
   */
  const handleHandleKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!runId || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) return;
    event.preventDefault();
    const target = event.key === "ArrowUp" ? props.position - 1 : props.position + 1;
    if (target < 0 || target >= props.total) return;
    void perform("move", () => props.onMove(runId, target)).catch(() => undefined);
  };

  return (
    <article
      className={`queued-message-row${props.dragging ? " is-dragging" : ""}`}
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault();
        props.onDrop();
      }}
    >
      <Button
        className="queued-message-handle"
        draggable
        onDragStart={(event) => {
          event.dataTransfer.effectAllowed = "move";
          event.dataTransfer.setData("text/plain", runId ?? "");
          props.onDragStart();
        }}
        onDragEnd={props.onDragEnd}
        onKeyDown={handleHandleKeyDown}
        disabled={!runId || busy !== null}
        aria-label={t(`Reorder message ${props.position + 1}`, `调整第 ${props.position + 1} 条消息顺序`)}
        title={t("Drag to reorder; use arrow keys for precise movement", "拖动排序；方向键微调")}
      >
        <GripVertical size={15} />
      </Button>

      <div className="queued-message-content">
        {editing ? (
          <TextArea
            ref={editorRef}
            className="queued-message-editor"
            value={draft}
            rows={2}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={handleEditorKeyDown}
            aria-label={t("Edit queued message", "编辑排队消息")}
          />
        ) : (
          <UserMessageBubble
            content={props.run.userInput}
            imageUrls={props.run.imageUrls}
          />
        )}
      </div>

      <div className="queued-message-actions">
        {editing ? (
          <>
            <Button
              className="queued-message-icon-action"
              onClick={() => void save()}
              disabled={busy !== null || (!draft.trim() && props.run.imageUrls.length === 0)}
              aria-label={t("Save queued message", "保存排队消息")}
              title={t("Save", "保存")}
            >
              <Check size={15} />
            </Button>
            <Button
              className="queued-message-icon-action"
              onClick={() => {
                setDraft(props.run.userInput);
                setEditing(false);
              }}
              disabled={busy !== null}
              aria-label={t("Cancel editing", "取消编辑")}
              title={t("Cancel", "取消")}
            >
              <X size={15} />
            </Button>
          </>
        ) : (
          <>
            <Button
              className="queued-message-promote"
              onClick={() => runId && void perform("move", () => props.onMove(runId, 0)).catch(() => undefined)}
              disabled={!runId || props.position === 0 || busy !== null}
              aria-label={t("Run next after the current task", "当前任务结束后立即执行")}
              title={t("Move to the front of the queue", "移到队首，当前任务结束后优先执行")}
            >
              <ArrowUpToLine size={14} />
              <span>{t("Next", "立即")}</span>
            </Button>
            <Button
              className="queued-message-icon-action"
              onClick={() => setEditing(true)}
              disabled={!runId || busy !== null}
              aria-label={t("Edit queued message", "编辑排队消息")}
              title={t("Edit", "编辑")}
            >
              <Pencil size={14} />
            </Button>
            <Button
              variant="ghost-danger"
              className="queued-message-icon-action"
              onClick={() => runId && void perform("remove", () => props.onRemove(runId)).catch(() => undefined)}
              disabled={!runId || busy !== null}
              aria-label={t("Delete queued message", "删除排队消息")}
              title={t("Delete", "删除")}
            >
              <Trash2 size={14} />
            </Button>
          </>
        )}
      </div>
    </article>
  );
}
