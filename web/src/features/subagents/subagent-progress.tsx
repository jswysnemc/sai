import { Loader2 } from "lucide-react";
import type { Subagent } from "../../api/contracts";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染运行中子智能体的实时进度:步数进度条与当前阶段。
 *
 * @param props 子智能体快照
 * @returns 进度视图,非运行态返回 null
 */
export function SubagentProgress({ subagent }: { subagent: Subagent }) {
  const { t } = useI18n();
  if (subagent.status !== "running") return null;
  const ratio = subagent.max_steps > 0 ? Math.min(1, subagent.step / subagent.max_steps) : 0;
  const phase = subagent.phase?.trim();
  return (
    <div className="subagent-progress">
      <div className="subagent-progress-head">
        <Loader2 size={12} className="spin" />
        <span className="subagent-progress-phase">{phase || t("Thinking", "正在思考")}</span>
        <span className="subagent-progress-step">{t(`${subagent.step}/${subagent.max_steps} steps`, `${subagent.step}/${subagent.max_steps} 步`)}</span>
      </div>
      <div className="subagent-progress-track" role="progressbar" aria-valuenow={subagent.step} aria-valuemin={0} aria-valuemax={subagent.max_steps}>
        <span className="subagent-progress-fill" style={{ width: `${Math.round(ratio * 100)}%` }} />
      </div>
    </div>
  );
}
