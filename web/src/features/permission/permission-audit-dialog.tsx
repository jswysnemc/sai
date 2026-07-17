import { useQuery } from "@tanstack/react-query";
import { ShieldCheck } from "lucide-react";
import { useState } from "react";
import { api } from "../../api/client";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import "./permission-audit-dialog.css";
import { useI18n } from "../i18n/use-i18n";

type PermissionAuditDialogProps = {
  sessionId?: string;
};

/**
 * 渲染当前会话的权限审计入口和事件列表。
 *
 * @param props 当前会话标识
 * @returns 权限审计按钮和对话框
 */
export function PermissionAuditDialog({ sessionId }: PermissionAuditDialogProps) {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(false);
  const audit = useQuery({
    queryKey: ["permission-audit", sessionId],
    queryFn: () => api.sessions.permissionAudit(sessionId!),
    enabled: open && Boolean(sessionId)
  });

  return (
    <>
      <Button
        className="composer-rail-button"
        disabled={!sessionId}
        onClick={() => setOpen(true)}
        title={t("View permission audit", "查看权限审计")}
        aria-label={t("View permission audit", "查看权限审计")}
      >
        <ShieldCheck size={14} />
      </Button>
      <Modal open={open} title={t("Permission audit", "权限审计")} description={t("Recent tool permission decisions and execution results for the current session.", "当前会话最近的工具权限判定和执行结果。")} onClose={() => setOpen(false)}>
        <div className="permission-audit-list">
          {audit.isLoading && <div className="permission-audit-empty">{t("Loading audit records", "正在读取审计记录")}</div>}
          {!audit.isLoading && audit.data?.length === 0 && <div className="permission-audit-empty">{t("No audit records", "暂无审计记录")}</div>}
          {audit.data?.map((event, index) => (
            <article className="permission-audit-event" key={`${event.timestamp_ms}-${event.tool}-${index}`}>
              <div className="permission-audit-event-head">
                <span>{event.tool}</span>
                <span className={`permission-audit-decision is-${event.decision}`}>{decisionLabel(event.decision, t)}</span>
              </div>
              <time>{new Date(event.timestamp_ms).toLocaleString(locale)}</time>
              {event.detail && <pre>{event.detail}</pre>}
            </article>
          ))}
        </div>
      </Modal>
    </>
  );
}

/**
 * 将审计判定转换为中文标签。
 *
 * @param decision 审计判定值
 * @returns 中文判定标签
 */
function decisionLabel(decision: "requested" | "approved" | "allowed" | "denied" | "completed" | "failed", t: (en: string, zh: string) => string) {
  return {
    requested: t("Requested", "待审批"),
    approved: t("Approved", "已批准"),
    allowed: t("Allowed", "允许"),
    denied: t("Denied", "拒绝"),
    completed: t("Completed", "完成"),
    failed: t("Failed", "失败")
  }[decision];
}
