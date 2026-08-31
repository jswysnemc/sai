import { Ban } from "lucide-react";
import type { KeyboardEvent, MouseEvent } from "react";
import type { Subagent } from "../../api/contracts";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { SubagentStatusBadge } from "./subagent-status-badge";
import { subagentDuration, subagentTypeLabel } from "./subagent-labels";
import { useI18n } from "../i18n/use-i18n";

type SubagentCardProps = {
  subagent: Subagent;
  active?: boolean;
  onSelect: () => void;
  onCancel: () => void;
};

/**
 * 渲染单个子智能体卡片:状态、类型与取消操作。
 *
 * @param props 子智能体数据、选中态与操作回调
 * @returns 子智能体卡片
 */
export function SubagentCard({ subagent, active = false, onSelect, onCancel }: SubagentCardProps) {
  const { locale, t } = useI18n();
  const confirm = useConfirm();
  /**
   * 阻止取消操作冒泡到卡片选择，并先确认。
   *
   * @param event 鼠标点击事件
   */
  const handleCancel = async (event: MouseEvent) => {
    event.stopPropagation();
    const accepted = await confirm({
      title: t("Cancel this subagent?", "取消这个子智能体？"),
      description: t("The subagent will stop. Unsent messages are discarded.", "子智能体将停止，未发送的留言会被丢弃。"),
      confirmLabel: t("Cancel subagent", "取消子智能体"),
      cancelLabel: t("Keep running", "继续运行"),
      danger: true
    });
    if (accepted) onCancel();
  };
  const handleKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onSelect();
    }
  };
  const running = subagent.status === "running";
  // 待命中的持久子智能体仍然存活,允许取消并提示未读留言
  const alive = running || subagent.status === "idle";
  const pending = subagent.pending_messages ?? 0;
  return (
    <article className={active ? "subagent-card active" : "subagent-card"}>
      <div
        className="subagent-card-main"
        role="button"
        tabIndex={0}
        onClick={onSelect}
        onKeyDown={handleKeyDown}
      >
        <div className="subagent-heading">
          <SubagentStatusBadge status={subagent.status} />
          <strong>{subagent.description}</strong>
          {pending > 0 && <span className="subagent-pending-count">{t(`${pending} queued`, `${pending} 条待注入`)}</span>}
        </div>
        <dl>
          <div><dt>{t("Type", "类型")}</dt><dd>{subagentTypeLabel(subagent.subagent_type, locale)}</dd></div>
          <div><dt>{t("Duration", "用时")}</dt><dd>{subagentDuration(subagent.started_at, subagent.updated_at)}</dd></div>
          {subagent.last_tool && !running && <div><dt>{t("Last step", "末步")}</dt><dd>{subagent.last_tool}</dd></div>}
        </dl>
        {subagent.error && <p className="subagent-error">{subagent.error}</p>}
      </div>
      {alive && (
        <button type="button" className="subagent-cancel" onClick={(event) => void handleCancel(event)}><Ban size={13} />{t("Cancel", "取消")}</button>
      )}
    </article>
  );
}
