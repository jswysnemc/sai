import type { SystemUsage } from "../../api/contracts";
import { useI18n } from "../i18n/use-i18n";
import { formatBytes, formatDuration } from "./usage-format";

/**
 * 展示独立的进程与运行时资源，避免挤占上下文弹层。
 * @param props 系统用量快照
 * @returns 进程资源列表
 */
export function ProcessUsagePanel({ usage }: { usage: SystemUsage }) {
  const { locale, t } = useI18n();
  return <section className="process-usage-panel">
    <dl className="process-usage-metrics">
      <div><dt>{t("CPU", "CPU 占用")}</dt><dd>{usage.process.cpu_percent.toFixed(1)}%</dd></div>
      <div><dt>{t("Resident memory", "常驻内存")}</dt><dd>{formatBytes(usage.process.rss_bytes, t)}</dd></div>
      <div><dt>{t("Terminals", "终端数量")}</dt><dd>{usage.runtime.terminal_count}</dd></div>
      <div><dt>{t("Current session", "当前会话")}</dt><dd>{usage.runtime.active_run ? t("Working", "正在工作") : t("Idle", "空闲")}</dd></div>
    </dl>
    <footer><span>PID {usage.process.pid}</span><span>{formatDuration(usage.process.uptime_seconds, locale)}</span></footer>
  </section>;
}
