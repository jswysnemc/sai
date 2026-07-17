import { CircleStop, LoaderCircle, Play } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";

type GatewayRuntimeControlProps = {
  gatewayId: "qq" | "weixin";
  enabled: boolean;
  dirty: boolean;
  onSave: () => Promise<void>;
};

/**
 * 渲染网关运行状态，并支持保存配置后启动或直接停止。
 *
 * @param props 网关标识、启用状态和保存回调
 * @returns 网关运行控制区
 */
export function GatewayRuntimeControl({ gatewayId, enabled, dirty, onSave }: GatewayRuntimeControlProps) {
  const queryClient = useQueryClient();
  const gateways = useQuery({ queryKey: ["gateways"], queryFn: api.gateways.list, refetchInterval: 5_000 });
  const status = gateways.data?.find((gateway) => gateway.id === gatewayId);
  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: ["gateways"] });
  };
  const start = useMutation({ mutationFn: api.gateways.start, onSuccess: refresh });
  const stop = useMutation({ mutationFn: api.gateways.stop, onSuccess: refresh });
  const running = status?.status === "running";
  const pending = start.isPending || stop.isPending;

  /** 保存未提交配置并启动当前网关。 */
  const handleStart = async () => {
    if (dirty) await onSave();
    await start.mutateAsync(gatewayId);
  };

  return (
    <div className="gateway-runtime">
      <div className={running ? "gateway-runtime-state running" : "gateway-runtime-state"}>
        <i />
        <span>{running ? "运行中" : enabled ? "已启用，未运行" : "配置未启用"}</span>
        {status?.pid && <small>PID {status.pid}</small>}
      </div>
      {running ? (
        <button type="button" className="gateway-runtime-button stop" onClick={() => stop.mutate(gatewayId)} disabled={pending}>{pending ? <LoaderCircle size={14} className="spin" /> : <CircleStop size={14} />}停止</button>
      ) : (
        <button type="button" className="gateway-runtime-button" onClick={() => void handleStart()} disabled={!enabled || pending}>{pending ? <LoaderCircle size={14} className="spin" /> : <Play size={14} />}{dirty ? "保存并启动" : "启动网关"}</button>
      )}
      {(gateways.error || start.error || stop.error) && <div className="gateway-runtime-error">{(gateways.error ?? start.error ?? stop.error)?.message}</div>}
    </div>
  );
}
