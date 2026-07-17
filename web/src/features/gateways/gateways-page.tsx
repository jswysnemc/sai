import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Cable, CircleStop, ExternalLink, Play, RefreshCw } from "lucide-react";
import { Link } from "react-router-dom";
import { api } from "../../api/client";
import type { GatewayStatus } from "../../api/contracts";
import "../settings/settings-layout.css";
import "./gateways-page.css";

/**
 * 渲染网关管理页，顶部复用设置页风格的返回主界面条。
 *
 * @returns 网关管理页面
 */
export function GatewaysPage() {
  const queryClient = useQueryClient();
  const gateways = useQuery({ queryKey: ["gateways"], queryFn: api.gateways.list, refetchInterval: 5_000 });
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["gateways"] });
  const start = useMutation({ mutationFn: api.gateways.start, onSuccess: refresh });
  const stop = useMutation({ mutationFn: api.gateways.stop, onSuccess: refresh });
  return (
    <div className="management-page">
      <header className="settings-topbar">
        <div className="settings-topbar-inner">
          <Link to="/" className="settings-back" aria-label="返回主界面"><ArrowLeft size={15} /><span>返回主界面</span></Link>
          <h1>网关</h1>
          <p>查看 QQ 与微信网关的配置和运行进程。</p>
          <div className="settings-topbar-actions">
            <button type="button" className="button refresh-button" onClick={() => void gateways.refetch()}><RefreshCw size={14} />刷新</button>
          </div>
        </div>
      </header>
      <div className="management-page-body">
        <header className="management-hero">
          <div className="hero-icon"><Cable size={24} /></div>
          <div><span className="eyebrow">Messaging gateways</span><h1>网关管理</h1><p>在网页中启动或停止受管理任务。</p></div>
        </header>
        <div className="gateway-grid">
          {gateways.data?.map((gateway) => <GatewayCard key={gateway.id} gateway={gateway} pending={start.isPending || stop.isPending} onStart={() => start.mutate(gateway.id)} onStop={() => stop.mutate(gateway.id)} />)}
        </div>
        <div className="gateway-config-link"><span>网关凭据和监听地址属于 Sai 配置。</span><Link to="/settings">打开配置管理 <ExternalLink size={13} /></Link></div>
        {(gateways.error || start.error || stop.error) && <div className="settings-error gateway-error">{(gateways.error ?? start.error ?? stop.error)?.message}</div>}
      </div>
    </div>
  );
}

function GatewayCard({ gateway, pending, onStart, onStop }: { gateway: GatewayStatus; pending: boolean; onStart: () => void; onStop: () => void }) {
  const running = gateway.status === "running";
  return (
    <article className={running ? "gateway-card running" : "gateway-card"}>
      <div className="gateway-card-top"><span className="gateway-index">{gateway.id.toUpperCase()}</span><span className={running ? "gateway-state running" : "gateway-state"}><i />{running ? "运行中" : gateway.status}</span></div>
      <h2>{gateway.title}</h2>
      <dl>
        <div><dt>配置</dt><dd>{gateway.enabled ? "已启用" : "未启用"}</dd></div>
        <div><dt>任务 ID</dt><dd>{gateway.task_id || "无"}</dd></div>
        <div><dt>PID</dt><dd>{gateway.pid ?? "无"}</dd></div>
      </dl>
      <button type="button" className={running ? "gateway-action stop" : "gateway-action"} onClick={running ? onStop : onStart} disabled={pending || (!gateway.enabled && !running)}>
        {running ? <CircleStop size={15} /> : <Play size={15} />}{running ? "停止网关" : "启动网关"}
      </button>
    </article>
  );
}
