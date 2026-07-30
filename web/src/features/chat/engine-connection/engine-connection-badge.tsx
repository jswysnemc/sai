import { Loader2, Plug, TriangleAlert } from "lucide-react";
import type { EngineStatusResponse } from "../../../api/contracts";
import { AgentEngineBrandIcon } from "../../../shared/ui/agent-engine-brand-icon/agent-engine-brand-icon";
import { useI18n } from "../../i18n/use-i18n";
import { engineConnectionLabel } from "./engine-connection-state";
import { useEngineConnection } from "./use-engine-connection";
import "./engine-connection.css";

type EngineConnectionBadgeProps = {
  /** 当前外部内核运行状态 */
  status: EngineStatusResponse;
  /** 本轮对话是否进行中 */
  running: boolean;
};

/**
 * 渲染外部内核连接徽标。
 *
 * 未连接时徽标是可点击的连接入口；连接完成后转为只读展示，
 * 由上层换成模型与思考等级选择器。
 *
 * @param props 内核状态与运行标记
 * @returns 连接徽标
 */
export function EngineConnectionBadge({ status, running }: EngineConnectionBadgeProps) {
  const { t } = useI18n();
  const connection = useEngineConnection(status);
  const text = engineConnectionLabel(connection.state, status.label);
  const busy = connection.state === "connecting";
  // 已连接时点击断开，其余状态点击发起握手
  const action = connection.state === "connected" ? connection.disconnect : connection.connect;
  const title = connection.error
    ? connection.error
    : t(
        connection.state === "connected"
          ? `Connected to ${status.label}; click to disconnect`
          : `Connect ${status.label} to load its models`,
        connection.state === "connected"
          ? `已连接 ${status.label}，点击断开`
          : `连接 ${status.label} 以读取其模型列表`
      );

  return (
    <button
      type="button"
      className={`composer-engine-badge engine-connection-badge is-${connection.state}`}
      onClick={action}
      disabled={busy || running}
      title={title}
      aria-label={t(text.en, text.zh)}
    >
      <ConnectionIcon state={connection.state} engine={status.engine} />
      {t(text.en, text.zh)}
    </button>
  );
}

/**
 * 按连接状态选择徽标图标。
 *
 * @param props 连接状态与内核标识
 * @returns 状态图标
 */
function ConnectionIcon({
  state,
  engine
}: {
  state: ReturnType<typeof useEngineConnection>["state"];
  engine: EngineStatusResponse["engine"];
}) {
  if (state === "connecting") {
    return <Loader2 size={12} className="engine-connection-spinner" aria-hidden />;
  }
  if (state === "failed") {
    return <TriangleAlert size={12} aria-hidden />;
  }
  if (state === "connected") {
    return <AgentEngineBrandIcon engine={engine} size={12} />;
  }
  return <Plug size={12} aria-hidden />;
}
