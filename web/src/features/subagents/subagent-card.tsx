import { Ban } from "lucide-react";
import type { MouseEvent } from "react";
import type { Subagent } from "../../api/contracts";
import { SubagentProgress } from "./subagent-progress";
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
 * 渲染单个子智能体卡片:状态、类型、实时进度与取消操作。
 *
 * @param props 子智能体数据、选中态与操作回调
 * @returns 子智能体卡片
 */
export function SubagentCard({ subagent, active = false, onSelect, onCancel }: SubagentCardProps) {
  const { locale, t } = useI18n();
  /**
   * 阻止取消操作冒泡到卡片选择。
   *
   * @param event 鼠标点击事件
   */
  const handleCancel = (event: MouseEvent) => {
    event.stopPropagation();
    onCancel();
  };
  const running = subagent.status === "running";
  return (
    <article className={active ? "subagent-card active" : "subagent-card"} onClick={onSelect}>
      <div className="subagent-heading">
        <SubagentStatusBadge status={subagent.status} />
        <strong>{subagent.description}</strong>
      </div>
      <dl>
        <div><dt>{t("Type", "类型")}</dt><dd>{subagentTypeLabel(subagent.subagent_type, locale)}</dd></div>
        <div><dt>{t("Duration", "用时")}</dt><dd>{subagentDuration(subagent.started_at, subagent.updated_at)}</dd></div>
        {subagent.last_tool && !running && <div><dt>{t("Last step", "末步")}</dt><dd>{subagent.last_tool}</dd></div>}
      </dl>
      <SubagentProgress subagent={subagent} />
      {subagent.error && <p className="subagent-error">{subagent.error}</p>}
      {running && (
        <button type="button" className="subagent-cancel" onClick={handleCancel}><Ban size={13} />{t("Cancel", "取消")}</button>
      )}
    </article>
  );
}
