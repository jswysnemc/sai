import type { SandboxStatusResponse } from "../../../../api/contracts";
import { Button } from "../../../../shared/ui/button/button";
import { useI18n } from "../../../i18n/use-i18n";
import { InlineNotice, StatusBadge, type StatusTone } from "../../kit";
import "./sandbox-settings.css";

type SandboxStatusSummaryProps = {
  status?: SandboxStatusResponse;
  loading: boolean;
  failed: boolean;
  onRetry: () => void;
};

/**
 * 【Web 设置】【沙箱状态】根据探测结果给出结论徽标、原因提示与路径清单。
 * @param props 探测结果、加载与失败状态、重试回调
 * @returns 状态摘要
 */
export function SandboxStatusSummary({ status, loading, failed, onRetry }: SandboxStatusSummaryProps) {
  const { t } = useI18n();
  if (loading) return <StatusBadge dot>{t("Detecting sandbox…", "正在探测沙箱…")}</StatusBadge>;
  if (failed || !status) {
    return (
      <div className="sandbox-status-row">
        <StatusBadge tone="danger" dot>{t("Detection failed", "探测失败")}</StatusBadge>
        <Button type="button" variant="secondary" size="small" onClick={onRetry}>{t("Retry", "重试")}</Button>
      </div>
    );
  }
  const verdict = sandboxVerdict(status, t);
  return (
    <div className="sandbox-status">
      <div className="sandbox-status-row">
        <StatusBadge tone={verdict.tone} dot>{verdict.label}</StatusBadge>
        {status.backend !== "none" && <StatusBadge mono>{status.backend}</StatusBadge>}
        <StatusBadge tone={status.network === "allow" ? "warning" : "neutral"}>
          {status.network === "allow" ? t("Network allowed", "允许网络") : t("Network blocked", "断开网络")}
        </StatusBadge>
      </div>
      {verdict.notice && <InlineNotice tone={verdict.tone === "danger" ? "danger" : "warning"}>{verdict.notice}</InlineNotice>}
      <dl className="sandbox-paths">
        <SandboxPathRow label={t("Writable", "可写")} items={status.writable_roots} />
        <SandboxPathRow label={t("Read-only inside writable", "可写目录中的只读路径")} items={status.write_protected} />
        <SandboxPathRow label={t("Hidden", "隐藏")} items={status.deny_read} />
        <SandboxPathRow label={t("Scrubbed env", "已移除的环境变量")} items={status.scrub_env ? status.scrubbed_env : []} empty={status.scrub_env ? "-" : t("Off", "关闭")} />
      </dl>
    </div>
  );
}

/**
 * 渲染一行路径清单。
 * @param props 标签、条目与空值文本
 * @returns 描述列表行
 */
function SandboxPathRow({ label, items, empty = "-" }: { label: string; items: string[]; empty?: string }) {
  return (
    <div className="sandbox-path-row">
      <dt>{label}</dt>
      <dd>{items.length ? items.map((item) => <code key={item}>{item}</code>) : <span className="sandbox-path-empty">{empty}</span>}</dd>
    </div>
  );
}

/**
 * 把探测结果归纳为结论、色调与提示。
 * @param status 探测结果
 * @param t 双语文本函数
 * @returns 结论标签、色调与可选提示
 */
export function sandboxVerdict(
  status: SandboxStatusResponse,
  t: (en: string, zh: string) => string
): { label: string; tone: StatusTone; notice?: string } {
  if (!status.platform_supported) {
    return {
      label: t("Not supported", "当前平台不支持"),
      tone: "warning",
      notice: t("This platform has no sandbox backend; audited commands run after approval without isolation.", "当前平台没有沙箱实现；审核模式的命令经批准后直接执行。")
    };
  }
  if (!status.enabled) {
    return {
      label: t("Off", "已关闭"),
      tone: "warning",
      notice: t("Audited commands run after approval without isolation.", "审核模式的命令经批准后直接执行，不做隔离。")
    };
  }
  if (!status.available) {
    return {
      label: t("Backend unavailable", "后端不可用"),
      tone: "danger",
      notice: `${status.reason ?? ""} ${t("Audited commands will fail until the backend works or the sandbox is turned off.", "修复后端或关闭沙箱之前，审核模式的命令会执行失败。")}`.trim()
    };
  }
  return { label: t("Active", "已生效"), tone: "success" };
}
