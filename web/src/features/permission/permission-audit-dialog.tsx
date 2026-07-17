import { useQuery } from "@tanstack/react-query";
import { ShieldCheck } from "lucide-react";
import { useState } from "react";
import { api } from "../../api/client";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import "./permission-audit-dialog.css";

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
        title="查看权限审计"
        aria-label="查看权限审计"
      >
        <ShieldCheck size={14} />
      </Button>
      <Modal open={open} title="权限审计" description="当前会话最近的工具权限判定和执行结果。" onClose={() => setOpen(false)}>
        <div className="permission-audit-list">
          {audit.isLoading && <div className="permission-audit-empty">正在读取审计记录</div>}
          {!audit.isLoading && audit.data?.length === 0 && <div className="permission-audit-empty">暂无审计记录</div>}
          {audit.data?.map((event, index) => (
            <article className="permission-audit-event" key={`${event.timestamp_ms}-${event.tool}-${index}`}>
              <div className="permission-audit-event-head">
                <span>{event.tool}</span>
                <span className={`permission-audit-decision is-${event.decision}`}>{decisionLabel(event.decision)}</span>
              </div>
              <time>{new Date(event.timestamp_ms).toLocaleString()}</time>
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
function decisionLabel(decision: "requested" | "approved" | "allowed" | "denied" | "completed" | "failed") {
  return {
    requested: "待审批",
    approved: "已批准",
    allowed: "允许",
    denied: "拒绝",
    completed: "完成",
    failed: "失败"
  }[decision];
}
