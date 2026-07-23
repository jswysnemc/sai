import { Check, ChevronDown, ShieldAlert, X } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { PermissionDecision, PermissionRequest } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { TextArea } from "../../shared/ui/form/text-area";
import { toolCardSummary } from "../chat/tool-renderers/tool-card-summary";
import { PermissionArgumentDetails } from "./permission-argument-details";
import "./permission-request-card.css";
import { useI18n } from "../i18n/use-i18n";

type PermissionRequestCardProps = {
  request: PermissionRequest;
  decision?: PermissionDecision;
  active?: boolean;
};

type PermissionCardStatus = "pending" | "allowed" | "auto_allowed" | "denied";

/**
 * 在助手 Markdown 流内渲染可交互权限请求。
 *
 * @param props 权限请求
 * @returns 内嵌权限审核卡片
 */
export function PermissionRequestCard({ request, decision, active = true }: PermissionRequestCardProps) {
  const { t } = useI18n();
  const [status, setStatus] = useState<PermissionCardStatus>(() => decisionStatus(decision));
  const [expanded, setExpanded] = useState(true);
  const [replyOpen, setReplyOpen] = useState(false);
  const [reply, setReply] = useState(() => decision?.decision === "deny" ? decision.reply ?? "" : "");
  const [submitting, setSubmitting] = useState<"allow" | "deny" | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const summary = toolCardSummary(request.tool, request.arguments) || request.tool.replaceAll("_", " ");

  useEffect(() => {
    setStatus(decisionStatus(decision));
    setExpanded(true);
    setReplyOpen(false);
    setReply(decision?.decision === "deny" ? decision.reply ?? "" : "");
    setSubmitting(null);
    setError(null);
  }, [request.id, decision]);

  /**
   * 提交允许或拒绝决定，并在原位置保留处理结果。
   *
   * @param decision 权限决定
   * @param includeReply 是否附带拒绝回复
   * @returns 提交完成后的 Promise
   */
  const decide = async (decision: "allow" | "deny", includeReply = false) => {
    setSubmitting(decision);
    setError(null);
    try {
      await api.permissions.decide(request, decision, decision === "deny" && includeReply ? reply.trim() || undefined : undefined);
      setStatus(decision === "allow" ? "allowed" : "denied"); // 人工按钮仅提交 human allow
      setExpanded(false);
    } catch (cause) {
      setError(toDisplayError(cause, "Failed to submit permission decision", "提交权限决定失败"));
    } finally {
      setSubmitting(null);
    }
  };

  const resolved = status !== "pending";
  const interactive = !resolved && active;
  return (
    <section className={`permission-request-card is-${status}`}>
      <Button className="permission-request-head" onClick={() => setExpanded((value) => !value)} aria-expanded={expanded}>
        <span className="permission-request-icon" aria-hidden>
          {status === "allowed" || status === "auto_allowed" ? <Check size={14} /> : status === "denied" ? <X size={14} /> : <ShieldAlert size={14} />}
        </span>
        <span className="permission-request-copy">
          <strong>{statusLabel(status, active, t)}</strong>
          <span title={summary}>{actionLabel(request.tool, t)} · {summary}</span>
          {status === "pending" && request.auto_audit ? (
            <span className="permission-auto-audit-badge">{t("Auto audit running", "自动审核进行中")}</span>
          ) : null}
        </span>
        <ChevronDown size={14} className={expanded ? "rotate" : ""} aria-hidden />
      </Button>
      {expanded && (
        <div className="permission-request-body">
          {status === "pending" && request.auto_audit ? (
            <div className="permission-auto-audit-hint">{t("LLM auto-audit is running in parallel. Your decision wins if submitted first; auto-audit timeout falls back to human review silently.", "LLM 自动审核并行进行中。人工先提交则优先生效；自动审核超时将静默回退人工审核。")}</div>
          ) : null}
          <PermissionArgumentDetails tool={request.tool} argumentsText={request.arguments} />
          {interactive && (
            <div className="permission-request-actions">
              {replyOpen && (
                <label className="permission-request-inline-reply">
                  <span>{t("Tell Sai how to adjust", "告诉 Sai 应如何调整")}</span>
                  <TextArea value={reply} onChange={(event) => setReply(event.target.value)} placeholder={t("Explain the reason for denial or provide an alternative request", "说明拒绝原因或提供替代要求")} autoFocus />
                </label>
              )}
              {error && <div className="permission-request-error">{error.message}</div>}
              <div className="permission-request-buttons">
                <Button className="permission-action" disabled={Boolean(submitting)} onClick={() => void decide("deny")}>{submitting === "deny" ? t("Denying", "正在拒绝") : t("Deny", "拒绝")}</Button>
                <Button className="permission-action" disabled={Boolean(submitting)} onClick={() => setReplyOpen((value) => !value)}>{t("Deny with reply", "拒绝并回复")}</Button>
                <Button variant="primary" className="permission-action" disabled={Boolean(submitting)} onClick={() => void decide("allow")}>{submitting === "allow" ? t("Allowing", "正在允许") : t("Allow once", "允许一次")}</Button>
              </div>
              {replyOpen && (
                <Button className="permission-reply-submit" disabled={Boolean(submitting) || !reply.trim()} onClick={() => void decide("deny", true)}>{submitting === "deny" ? t("Submitting", "正在提交") : t("Submit denial reply", "提交拒绝回复")}</Button>
              )}
            </div>
          )}
          {!resolved && !active && <div className="permission-request-ended">{t("The permission request ended with this run", "权限请求已随本轮运行结束")}</div>}
          {resolved && reply.trim() && status === "denied" && <div className="permission-resolved-reply"><span>{t("Reply", "回复")}</span>{reply.trim()}</div>}
        </div>
      )}
    </section>
  );
}

/**
 * 返回工具权限动作标签。
 *
 * @param tool 工具名称
 * @param t 双语文本选择方法
 * @returns 用户可读动作
 */
function actionLabel(tool: string, t: (en: string, zh: string) => string): string {
  if (tool === "run_command" || tool.includes("background_command")) return t("Run command", "执行命令");
  if (tool === "edit_file") return t("Modify file", "修改文件");
  if (tool === "trash_path") return t("Move to trash", "移入回收站");
  return t("Run tool", "执行工具");
}

/**
 * 返回权限卡片状态标签。
 *
 * @param status 当前卡片状态
 * @param t 双语文本选择方法
 * @returns 状态标签
 */
function statusLabel(status: PermissionCardStatus, active: boolean, t: (en: string, zh: string) => string): string {
  if (status === "pending" && !active) return t("Request ended", "请求已结束");
  return {
    pending: t("Permission required", "需要权限"),
    allowed: t("Allowed once", "已允许一次"),
    auto_allowed: t("Auto-allowed once", "已自动允许一次"),
    denied: t("Denied", "已拒绝")
  }[status];
}

/**
 * 将后端权限决定转换为卡片状态。
 *
 * @param decision 可选权限决定
 * @returns 权限卡片状态
 */
function decisionStatus(decision?: PermissionDecision): PermissionCardStatus {
  if (!decision) return "pending";
  if (decision.decision === "deny") return "denied";
  if (decision.source === "auto_audit") return "auto_allowed";
  return "allowed";
}
