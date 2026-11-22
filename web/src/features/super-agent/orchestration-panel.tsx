import { CircleStop, FileText, ListChecks, RotateCcw } from "../../shared/ui/icons";
import type { SuperRunnerReport, SuperTask } from "../../api/super-agent-contracts";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";

type OrchestrationPanelProps = {
  tasks: SuperTask[];
  reports: SuperRunnerReport[];
  busy: boolean;
  onRetryTask: (taskId: string) => Promise<void>;
  onStopTask: (taskId: string) => Promise<void>;
};

/**
 * 【超级智能体】【任务与报告面板】展示 Leader 编排的任务工单生命周期与 Runner 提交的结构化工作报告。
 *
 * @param props 任务列表、报告列表与操作回调
 * @returns 任务与报告视图
 */
export function OrchestrationPanel({
  tasks,
  reports,
  busy,
  onRetryTask,
  onStopTask
}: OrchestrationPanelProps) {
  const { t } = useI18n();

  return (
    <div className="sa-panel-scroll">
      <section className="sa-section-card">
        <header className="sa-section-title">
          <span style={{ display: "inline-flex", alignItems: "center", gap: "0.4rem" }}>
            <ListChecks size={16} />
            {t("Orchestrated Tasks", "编排任务工单")} ({tasks.length})
          </span>
        </header>

        {tasks.length === 0 ? (
          <div className="sa-empty">
            {t("No tasks dispatched yet. Ask the Leader or spawn a Runner.", "暂无编排任务。可向 Leader 下达目标或直接启动 Runner。")}
          </div>
        ) : (
          <div className="sa-grid-2">
            {tasks.map((task) => {
              const canStop = task.status === "running" || task.status === "pending";
              const canRetry = task.status === "failed" || task.status === "blocked" || task.status === "stopped";
              return (
                <article key={task.id} className="sa-item-card">
                  <div className="sa-group-card-top">
                    <strong>{task.title}</strong>
                    <span className={task.status === "delivered" ? "sa-badge accent" : task.status === "failed" || task.status === "blocked" ? "sa-badge danger" : "sa-badge"}>
                      {task.status}
                    </span>
                  </div>
                  <div className="sa-group-meta">
                    <span>{t("Runner", "执行者")}: {task.assigned_member_name ?? "-"}</span>
                    <span>{t("Origin", "来源")}: {task.origin_channel}</span>
                  </div>
                  <p className="sa-msg-body" style={{ fontSize: "0.78rem", color: "var(--ink-soft)" }}>
                    {task.objective}
                  </p>
                  {task.latest_report_summary && (
                    <div className="sa-item-card" style={{ padding: "0.4rem 0.55rem" }}>
                      <span className="sa-group-meta">{t("Latest Runner Report", "最新 Runner 汇报")}</span>
                      <span style={{ fontSize: "0.78rem" }}>{task.latest_report_summary}</span>
                    </div>
                  )}
                  {task.deliverable && (
                    <div className="sa-item-card" style={{ padding: "0.4rem 0.55rem", borderColor: "var(--signal)" }}>
                      <span className="sa-group-meta">{t("Leader Synthesized Deliverable", "Leader 归纳交付成果")}</span>
                      <span style={{ fontSize: "0.78rem", whiteSpace: "pre-wrap" }}>{task.deliverable}</span>
                    </div>
                  )}
                  {(canStop || canRetry) && (
                    <div style={{ display: "flex", gap: "0.4rem", marginTop: "0.25rem" }}>
                      {canRetry && (
                        <Button size="small" variant="secondary" disabled={busy} onClick={() => void onRetryTask(task.id)}>
                          <RotateCcw size={14} />
                          {t("Retry", "重试")}
                        </Button>
                      )}
                      {canStop && (
                        <Button size="small" variant="ghost-danger" disabled={busy} onClick={() => void onStopTask(task.id)}>
                          <CircleStop size={14} />
                          {t("Stop", "停止")}
                        </Button>
                      )}
                    </div>
                  )}
                </article>
              );
            })}
          </div>
        )}
      </section>

      <section className="sa-section-card">
        <header className="sa-section-title">
          <span style={{ display: "inline-flex", alignItems: "center", gap: "0.4rem" }}>
            <FileText size={16} />
            {t("Runner Reports", "Runner 结构化成果报告")} ({reports.length})
          </span>
        </header>

        {reports.length === 0 ? (
          <div className="sa-empty">
            {t("Completed Runner reports appear here before and after Leader synthesis.", "Runner 完成后提交给 Leader 的结构化报告将在此展示。")}
          </div>
        ) : (
          <div className="sa-grid-2">
            {reports.map((report) => (
              <article key={report.id} className="sa-item-card">
                <div className="sa-group-card-top">
                  <strong>{report.worker_name}</strong>
                  <div style={{ display: "flex", gap: "0.3rem" }}>
                    <span className="sa-badge">{report.status}</span>
                    <span className={report.synthesized_by_leader ? "sa-badge accent" : "sa-badge"}>
                      {report.synthesized_by_leader
                        ? t("Synthesized", "已归纳交付")
                        : t("Awaiting Leader", "待 Leader 归纳")}
                    </span>
                  </div>
                </div>
                <div style={{ fontSize: "0.8125rem", fontWeight: 500 }}>{report.summary}</div>
                {report.details && report.details !== report.summary && (
                  <details>
                    <summary style={{ cursor: "pointer", fontSize: "0.75rem", color: "var(--ink-soft)" }}>
                      {t("View detailed report", "展开详细报告")}
                    </summary>
                    <p className="sa-msg-body" style={{ marginTop: "0.35rem", fontSize: "0.78rem" }}>
                      {report.details}
                    </p>
                  </details>
                )}
                {report.artifacts.length > 0 && (
                  <div className="sa-group-meta">
                    {t("Artifacts", "产物")}: {report.artifacts.join(", ")}
                  </div>
                )}
                {report.needs_human_question && (
                  <div className="sa-inline-error">
                    {t("Needs decision", "待确认问题")}: {report.needs_human_question}
                  </div>
                )}
              </article>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
