import type { EngineStatusResponse } from "../../../api/contracts";

/** 外部内核的连接状态 */
export type EngineConnectionState = "idle" | "connecting" | "connected" | "failed";

type ResolveInput = {
  /** 当前内核运行状态 */
  status: EngineStatusResponse | undefined;
  /** 连接请求是否进行中 */
  connecting: boolean;
  /** 上一次连接是否失败 */
  failed: boolean;
};

/**
 * 推导外部内核当前应展示的连接状态。
 *
 * 进行中的请求优先于任何缓存结果；已握手的运行状态视为已连接；
 * 失败标记仅在没有可用运行状态时才展示，避免重连成功后仍显示失败。
 *
 * @param input 运行状态与请求标记
 * @returns 界面展示用的连接状态
 */
export function resolveEngineConnectionState({
  status,
  connecting,
  failed
}: ResolveInput): EngineConnectionState {
  if (connecting) return "connecting";
  if (hasHandshake(status)) return "connected";
  return failed ? "failed" : "idle";
}

/**
 * 判断运行状态中是否已包含握手结果。
 *
 * @param status 当前内核运行状态
 * @returns 已完成握手时返回 true
 */
export function hasHandshake(status: EngineStatusResponse | undefined): boolean {
  return Boolean(status?.acp_runtime);
}

/**
 * 返回连接状态对应的双语文案。
 *
 * @param state 连接状态
 * @param label 内核展示名称
 * @returns 英文与中文文案
 */
export function engineConnectionLabel(
  state: EngineConnectionState,
  label: string
): { en: string; zh: string } {
  switch (state) {
    case "connecting":
      return { en: `Connecting to ${label}`, zh: `正在连接 ${label}` };
    case "connected":
      return { en: label, zh: label };
    case "failed":
      return { en: `${label} unavailable`, zh: `${label} 连接失败` };
    case "idle":
      return { en: `Connect ${label}`, zh: `连接 ${label}` };
  }
}
