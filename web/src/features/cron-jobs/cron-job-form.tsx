import { useEffect, useMemo, useState, type FormEvent } from "react";
import { CalendarPlus, LoaderCircle } from "lucide-react";
import type { CreateCronJobRequest, Session } from "../../api/contracts";
import { Select } from "../../shared/ui/select/select";
import { useI18n } from "../i18n/use-i18n";

type CronJobFormProps = {
  sessions: Session[];
  pending: boolean;
  onSubmit: (request: CreateCronJobRequest) => Promise<void>;
};

type ScheduleKind = "once" | "interval";

/**
 * 生成适用于日期时间输入框的默认执行时间。
 *
 * @returns 十分钟后的本地日期时间
 */
function defaultRunAt(): string {
  const date = new Date(Date.now() + 10 * 60 * 1_000);
  date.setSeconds(0, 0);
  const offset = date.getTimezoneOffset() * 60 * 1_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}

/**
 * 渲染定时任务创建表单。
 *
 * @param props 会话列表、提交状态和创建回调
 * @returns 定时任务创建表单
 */
export function CronJobForm({ sessions, pending, onSubmit }: CronJobFormProps) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [prompt, setPrompt] = useState("");
  const [sessionId, setSessionId] = useState("");
  const [scheduleKind, setScheduleKind] = useState<ScheduleKind>("once");
  const [runAt, setRunAt] = useState(defaultRunAt);
  const [intervalMinutes, setIntervalMinutes] = useState(60);
  const sessionOptions = useMemo(
    () => [
      { value: "", label: sessions.length === 0 ? t("No sessions available", "暂无可用会话") : t("Select a session", "选择会话") },
      ...sessions.map((session) => ({
        value: session.id,
        label: session.title,
        description: session.active ? t("Current session", "当前会话") : undefined
      }))
    ],
    [sessions, t]
  );

  useEffect(() => {
    if (!sessionId && sessions.length > 0) {
      setSessionId(sessions.find((session) => session.active)?.id ?? sessions[0].id);
    }
  }, [sessionId, sessions]);

  /** 将表单值转换为接口请求并完成创建。 */
  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const timestamp = Math.floor(new Date(runAt).getTime() / 1_000);
    if (!Number.isFinite(timestamp)) return;
    await onSubmit({
      name: name.trim(),
      prompt: prompt.trim(),
      session_id: sessionId,
      run_at: timestamp,
      interval_seconds: scheduleKind === "interval" ? intervalMinutes * 60 : null
    });
    setName("");
    setPrompt("");
    setRunAt(defaultRunAt());
  };

  const invalid = !name.trim() || !prompt.trim() || !sessionId || !runAt || (scheduleKind === "interval" && intervalMinutes < 1);

  return (
    <form className="cron-form" onSubmit={(event) => void handleSubmit(event)}>
      <div className="cron-section-heading">
        <CalendarPlus size={18} />
        <div><h2>{t("Create task", "创建任务")}</h2><p>{t("Tasks run through the active Gateway scheduler.", "任务由正在运行的 Gateway 调度器执行。")}</p></div>
      </div>
      <div className="cron-form-grid">
        <label><span>{t("Task name", "任务名称")}</span><input value={name} onChange={(event) => setName(event.target.value)} placeholder={t("For example: Daily project summary", "例如：每日项目摘要")} maxLength={120} /></label>
        <label>
          <span>{t("Target session", "目标会话")}</span>
          <Select
            value={sessionId}
            options={sessionOptions}
            disabled={sessions.length === 0}
            ariaLabel={t("Target session", "目标会话")}
            menuPreferredWidth={320}
            menuMinimumWidth={220}
            onChange={setSessionId}
          />
        </label>
        <label className="cron-field-wide"><span>{t("Execution prompt", "执行提示词")}</span><textarea value={prompt} onChange={(event) => setPrompt(event.target.value)} placeholder={t("Enter the instruction sent to the agent when the task runs", "输入任务执行时发送给智能体的指令")} rows={4} /></label>
        <fieldset className="cron-field-wide cron-schedule-field"><legend>{t("Schedule", "调度方式")}</legend><div className="cron-segmented"><button type="button" className={scheduleKind === "once" ? "active" : ""} onClick={() => setScheduleKind("once")}>{t("Once", "单次")}</button><button type="button" className={scheduleKind === "interval" ? "active" : ""} onClick={() => setScheduleKind("interval")}>{t("Fixed interval", "固定间隔")}</button></div></fieldset>
        <label><span>{scheduleKind === "once" ? t("Run time", "执行时间") : t("First run time", "首次执行时间")}</span><input type="datetime-local" value={runAt} onChange={(event) => setRunAt(event.target.value)} /></label>
        {scheduleKind === "interval" && <label><span>{t("Interval in minutes", "间隔分钟数")}</span><input type="number" min={1} step={1} value={intervalMinutes} onChange={(event) => setIntervalMinutes(Number(event.target.value))} /></label>}
      </div>
      <button type="submit" className="cron-primary-button" disabled={pending || invalid || sessions.length === 0}>{pending ? <LoaderCircle size={15} className="spin" /> : <CalendarPlus size={15} />}{t("Create task", "创建任务")}</button>
    </form>
  );
}
