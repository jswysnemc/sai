import { CalendarClock, LoaderCircle, Pause, Play, Trash2 } from "lucide-react";
import type { CronJob } from "../../api/contracts";
import { formatCronDate, formatCronInterval, getCronJobStatus } from "./cron-job-utils";

type CronJobListProps = {
  jobs: CronJob[];
  pendingId?: string;
  onToggle: (job: CronJob) => void;
  onRemove: (job: CronJob) => void;
};

/**
 * 渲染定时任务列表与操作入口。
 *
 * @param props 任务列表、操作状态和回调
 * @returns 定时任务列表
 */
export function CronJobList({ jobs, pendingId, onToggle, onRemove }: CronJobListProps) {
  if (jobs.length === 0) {
    return <div className="cron-empty"><CalendarClock size={24} /><strong>暂无定时任务</strong><span>创建单次任务或固定间隔任务后会显示在这里。</span></div>;
  }
  return <div className="cron-job-list">{jobs.map((job) => <CronJobRow key={job.id} job={job} pending={pendingId === job.id} onToggle={() => onToggle(job)} onRemove={() => onRemove(job)} />)}</div>;
}

/**
 * 渲染单个定时任务的状态与管理操作。
 *
 * @param props 任务数据、操作状态和回调
 * @returns 定时任务行
 */
function CronJobRow({ job, pending, onToggle, onRemove }: { job: CronJob; pending: boolean; onToggle: () => void; onRemove: () => void }) {
  return (
    <article className={job.enabled ? "cron-job-row enabled" : "cron-job-row"}>
      <div className="cron-job-state"><i /><span>{getCronJobStatus(job)}</span></div>
      <div className="cron-job-main"><div className="cron-job-title"><h3>{job.name}</h3><span>{formatCronInterval(job.interval_seconds)}</span></div><p>{job.prompt}</p><dl><div><dt>下次执行</dt><dd>{formatCronDate(job.next_run_at)}</dd></div><div><dt>目标会话</dt><dd title={job.session_id}>{job.session_id}</dd></div><div><dt>连续失败</dt><dd>{job.failure_count} 次</dd></div></dl>{job.last_error && <div className="cron-last-error">{job.last_error}</div>}</div>
      <div className="cron-job-actions"><button type="button" title={job.enabled ? "停用任务" : "启用任务"} aria-label={job.enabled ? `停用 ${job.name}` : `启用 ${job.name}`} onClick={onToggle} disabled={pending}>{pending ? <LoaderCircle size={16} className="spin" /> : job.enabled ? <Pause size={16} /> : <Play size={16} />}</button><button type="button" className="danger" title="删除任务" aria-label={`删除 ${job.name}`} onClick={onRemove} disabled={pending}><Trash2 size={16} /></button></div>
    </article>
  );
}
