import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { CheckCircle2, CircleAlert, Loader2, PlugZap } from "../../../shared/ui/icons";
import { api } from "../../../api/client";
import { toDisplayError } from "../../../api/api-error";
import type { JevProbeReport, JevStatus } from "../../../api/contracts/jev";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { JevProbeResult } from "./jev-probe-result";

/** 状态查询键；保存后按 dirty 变化重新读取。 */
export const JEV_STATUS_QUERY_KEY = "jev-status";

type JevConnectionStatusProps = {
  /** 全局草稿是否有未保存修改；由 true 变为 false 时重新读取状态 */
  dirty: boolean;
};

/**
 * 【Jev接入】【状态行】展示已保存配置下的生效接入与密钥状态，并测试该接入。
 * @param props 草稿修改状态
 * @returns 单行状态与可选的测试结果
 */
export function JevConnectionStatus({ dirty }: JevConnectionStatusProps) {
  const { t } = useI18n();
  const status = useQuery({ queryKey: [JEV_STATUS_QUERY_KEY, dirty], queryFn: api.jev.status });
  const [probing, setProbing] = useState(false);
  const [report, setReport] = useState<JevProbeReport | null>(null);
  const [error, setError] = useState("");

  /** 测试已保存的生效接入。 */
  const probe = async () => {
    setProbing(true);
    setError("");
    setReport(null);
    try {
      setReport(await api.jev.test());
    } catch (cause) {
      setError(toDisplayError(cause, "Jev connection test failed", "Jev 连接测试失败").message);
    } finally {
      setProbing(false);
    }
  };

  return (
    <div className="jev-status">
      <div className="jev-status-row">
        <StatusSummary status={status.data} loading={status.isLoading} />
        <Button className="settings-secondary" disabled={probing || !status.data?.key_ready} onClick={() => void probe()}>
          {probing ? <Loader2 size={14} className="spin" /> : <PlugZap size={14} />}
          {probing ? t("Testing", "测试中") : t("Test", "测试")}
        </Button>
      </div>
      {dirty && <small className="jev-status-note">{t("Status reflects the saved configuration. Save to apply changes.", "状态基于已保存的配置，保存后生效。")}</small>}
      {error && <p className="settings-inline-error">{error}</p>}
      {report && <JevProbeResult report={report} />}
    </div>
  );
}

/**
 * 接入与密钥的单行摘要。
 * @param props.status 状态；加载中为空
 * @param props.loading 是否正在读取
 * @returns 带状态图标的摘要
 */
function StatusSummary({ status, loading }: { status?: JevStatus; loading: boolean }) {
  const { t } = useI18n();
  if (loading || !status) {
    return <span className="jev-status-summary">{t("Reading status", "读取状态")}</span>;
  }
  const connection = status.connection;
  const source = connection?.source === "endpoint" ? connection.name : t("Official TypeSafe", "TypeSafe 官方");
  return (
    <span className={status.key_ready ? "jev-status-summary is-ready" : "jev-status-summary is-blocked"}>
      {status.key_ready ? <CheckCircle2 size={14} /> : <CircleAlert size={14} />}
      <strong>{source}</strong>
      {connection && <code title={connection.endpoint}>{connection.endpoint}</code>}
      {connection && <em>{connection.model}</em>}
      {status.error && <small title={status.error}>{status.error}</small>}
    </span>
  );
}
