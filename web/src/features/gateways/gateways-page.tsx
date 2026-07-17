import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Cable, CircleStop, ExternalLink, Play, RefreshCw } from "lucide-react";
import { Link } from "react-router-dom";
import { api } from "../../api/client";
import type { GatewayStatus } from "../../api/contracts";
import "../settings/settings-layout.css";
import "./gateways-page.css";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染网关管理页，顶部复用设置页风格的返回主界面条。
 *
 * @returns 网关管理页面
 */
export function GatewaysPage() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const gateways = useQuery({ queryKey: ["gateways"], queryFn: api.gateways.list, refetchInterval: 5_000 });
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["gateways"] });
  const start = useMutation({ mutationFn: api.gateways.start, onSuccess: refresh });
  const stop = useMutation({ mutationFn: api.gateways.stop, onSuccess: refresh });
  return (
    <div className="management-page">
      <header className="settings-topbar">
        <div className="settings-topbar-inner">
          <Link to="/" className="settings-back" aria-label={t("Back to workspace", "返回主界面")}><ArrowLeft size={15} /><span>{t("Back to workspace", "返回主界面")}</span></Link>
          <h1>{t("Gateways", "网关")}</h1>
          <p>{t("View QQ and Weixin gateway configuration and managed processes.", "查看 QQ 与微信网关的配置和运行进程。")}</p>
          <div className="settings-topbar-actions">
            <button type="button" className="button refresh-button" onClick={() => void gateways.refetch()}><RefreshCw size={14} />{t("Refresh", "刷新")}</button>
          </div>
        </div>
      </header>
      <div className="management-page-body">
        <header className="management-hero">
          <div className="hero-icon"><Cable size={24} /></div>
          <div><span className="eyebrow">{t("Messaging gateways", "消息网关")}</span><h1>{t("Gateway management", "网关管理")}</h1><p>{t("Start or stop managed gateway processes from the Web interface.", "在网页中启动或停止受管理任务。")}</p></div>
        </header>
        <div className="gateway-grid">
          {gateways.data?.map((gateway) => <GatewayCard key={gateway.id} gateway={gateway} pending={start.isPending || stop.isPending} onStart={() => start.mutate(gateway.id)} onStop={() => stop.mutate(gateway.id)} />)}
        </div>
        <div className="gateway-config-link"><span>{t("Gateway credentials and listen addresses are part of Sai configuration.", "网关凭据和监听地址属于 Sai 配置。")}</span><Link to="/settings">{t("Open settings", "打开配置管理")} <ExternalLink size={13} /></Link></div>
        {(gateways.error || start.error || stop.error) && <div className="settings-error gateway-error">{(gateways.error ?? start.error ?? stop.error)?.message}</div>}
      </div>
    </div>
  );
}

function GatewayCard({ gateway, pending, onStart, onStop }: { gateway: GatewayStatus; pending: boolean; onStart: () => void; onStop: () => void }) {
  const { t } = useI18n();
  const running = gateway.status === "running";
  return (
    <article className={running ? "gateway-card running" : "gateway-card"}>
      <div className="gateway-card-top"><span className="gateway-index">{gateway.id.toUpperCase()}</span><span className={running ? "gateway-state running" : "gateway-state"}><i />{running ? t("Running", "运行中") : gateway.status}</span></div>
      <h2>{gateway.title}</h2>
      <dl>
        <div><dt>{t("Configuration", "配置")}</dt><dd>{gateway.enabled ? t("Enabled", "已启用") : t("Disabled", "未启用")}</dd></div>
        <div><dt>{t("Task ID", "任务 ID")}</dt><dd>{gateway.task_id || t("None", "无")}</dd></div>
        <div><dt>PID</dt><dd>{gateway.pid ?? t("None", "无")}</dd></div>
      </dl>
      <button type="button" className={running ? "gateway-action stop" : "gateway-action"} onClick={running ? onStop : onStart} disabled={pending || (!gateway.enabled && !running)}>
        {running ? <CircleStop size={15} /> : <Play size={15} />}{running ? t("Stop gateway", "停止网关") : t("Start gateway", "启动网关")}
      </button>
    </article>
  );
}
