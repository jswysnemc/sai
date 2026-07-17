import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, CalendarClock, RefreshCw } from "lucide-react";
import { Link } from "react-router-dom";
import { api } from "../../api/client";
import type { CreateCronJobRequest, CronJob } from "../../api/contracts";
import { CronJobForm } from "./cron-job-form";
import { CronJobList } from "./cron-job-list";
import "../settings/settings-layout.css";
import "./cron-jobs.css";

/**
 * 渲染定时任务状态和管理页面。
 *
 * @returns 定时任务管理页面
 */
export function CronJobsPage() {
  const queryClient = useQueryClient();
  const [pendingId, setPendingId] = useState<string>();
  const jobs = useQuery({ queryKey: ["cron-jobs"], queryFn: api.cronJobs.list, refetchInterval: 5_000 });
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: api.sessions.list });

  /** 刷新定时任务列表。 */
  const refreshJobs = async () => {
    await queryClient.invalidateQueries({ queryKey: ["cron-jobs"] });
  };

  const create = useMutation({ mutationFn: api.cronJobs.create, onSuccess: refreshJobs });
  const update = useMutation({ mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) => api.cronJobs.update(id, { enabled }), onSuccess: refreshJobs, onSettled: () => setPendingId(undefined) });
  const remove = useMutation({ mutationFn: api.cronJobs.remove, onSuccess: refreshJobs, onSettled: () => setPendingId(undefined) });

  /** 创建新的定时任务。 */
  const handleCreate = async (request: CreateCronJobRequest) => {
    await create.mutateAsync(request);
  };

  /** 切换指定任务的启用状态。 */
  const handleToggle = (job: CronJob) => {
    setPendingId(job.id);
    update.mutate({ id: job.id, enabled: !job.enabled });
  };

  /** 删除指定任务。 */
  const handleRemove = (job: CronJob) => {
    if (!window.confirm(`确定删除定时任务“${job.name}”吗？`)) return;
    setPendingId(job.id);
    remove.mutate(job.id);
  };

  const error = jobs.error ?? sessions.error ?? create.error ?? update.error ?? remove.error;

  return (
    <div className="cron-page">
      <header className="settings-topbar">
        <div className="settings-topbar-inner">
          <Link to="/" className="settings-back" aria-label="返回主界面"><ArrowLeft size={15} /><span>返回主界面</span></Link>
          <h1>定时任务</h1>
          <p>创建任务并查看调度状态。</p>
          <div className="settings-topbar-actions">
            <button type="button" className="cron-refresh-button" onClick={() => void jobs.refetch()} disabled={jobs.isFetching}><RefreshCw size={15} className={jobs.isFetching ? "spin" : ""} />刷新</button>
          </div>
        </div>
      </header>
      <div className="cron-page-body">
        <header className="cron-hero"><div className="cron-hero-icon"><CalendarClock size={24} /></div><div><span className="cron-eyebrow">Gateway scheduler</span><h1>任务调度</h1><p>只有 Gateway 进程运行时才会执行到期任务。</p></div></header>
        <div className="cron-layout"><CronJobForm sessions={sessions.data ?? []} pending={create.isPending} onSubmit={handleCreate} /><section className="cron-list-panel"><div className="cron-section-heading"><CalendarClock size={18} /><div><h2>任务状态</h2><p>{jobs.data?.length ?? 0} 个任务，状态每 5 秒刷新。</p></div></div>{jobs.isLoading ? <div className="cron-loading"><LoaderLabel /></div> : <CronJobList jobs={jobs.data ?? []} pendingId={pendingId} onToggle={handleToggle} onRemove={handleRemove} />}</section></div>
        {error && <div className="cron-error">{error.message}</div>}
      </div>
    </div>
  );
}

/**
 * 渲染任务列表加载状态。
 *
 * @returns 加载文案
 */
function LoaderLabel() {
  return <span>正在读取任务</span>;
}
