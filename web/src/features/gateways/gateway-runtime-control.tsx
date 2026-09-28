import { CircleStop, LoaderCircle, Play } from "../../shared/ui/icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import { useI18n } from "../i18n/use-i18n";
import { Button } from "../../shared/ui/button/button";
import { StatusBadge, InlineNotice } from "../settings/kit";

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
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const gateways = useQuery({ queryKey: ["gateways"], queryFn: api.gateways.list, refetchInterval: 5_000 });
  const status = gateways.data?.find((gateway) => gateway.id === gatewayId);
  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: ["gateways"] });
  };
  const start = useMutation({
    mutationFn: async () => {
      // 1. 【消息网关】【启动】保存失败时保留错误并阻止启动
      if (dirty) await onSave();
      return api.gateways.start(gatewayId);
    },
    onSuccess: refresh
  });
  const stop = useMutation({ mutationFn: api.gateways.stop, onSuccess: refresh });
  const running = status?.status === "running";
  const pending = start.isPending || stop.isPending;

  return (
    <div className="flex flex-wrap items-center gap-2">
      <StatusBadge tone={running ? "success" : "neutral"} dot>
        <span>{running ? t("Running", "运行中") : enabled ? t("Enabled, not running", "已启用，未运行") : t("Configuration disabled", "配置未启用")}</span>
        {status?.pid && <small>PID {status.pid}</small>}
      </StatusBadge>
      {running ? (
        <Button size="small" onClick={() => stop.mutate(gatewayId)} disabled={pending}>{pending ? <LoaderCircle size={14} className="spin" /> : <CircleStop size={14} />}{t("Stop", "停止")}</Button>
      ) : (
        <Button size="small" onClick={() => start.mutate()} disabled={!enabled || pending}>{pending ? <LoaderCircle size={14} className="spin" /> : <Play size={14} />}{dirty ? t("Save and start", "保存并启动") : t("Start gateway", "启动网关")}</Button>
      )}
      {(gateways.error || start.error || stop.error) && <InlineNotice tone="danger">{(gateways.error ?? start.error ?? stop.error)?.message}</InlineNotice>}
    </div>
  );
}
