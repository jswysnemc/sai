import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../../api/client";
import { toDisplayError } from "../../../api/api-error";
import type { EngineStatusResponse } from "../../../api/contracts";
import {
  resolveEngineConnectionState,
  type EngineConnectionState
} from "./engine-connection-state";

type UseEngineConnectionResult = {
  /** 当前展示用的连接状态 */
  state: EngineConnectionState;
  /** 上一次连接失败的原因 */
  error: string;
  /** 发起握手 */
  connect: () => void;
  /** 丢弃已缓存的能力 */
  disconnect: () => void;
};

/**
 * 提取连接失败的可展示原因。
 *
 * @param cause 请求抛出的错误
 * @returns 可直接展示的消息
 */
function connectionErrorMessage(cause: unknown): string {
  return toDisplayError(
    cause,
    "Failed to connect the external engine",
    "连接外部内核失败"
  ).message;
}

/**
 * 管理外部内核的手动连接。
 *
 * 握手结果写在服务端全局运行状态里，因此成功或断开后都要让内核状态查询失效，
 * 由既有的 engine-status 查询把新的能力带回界面。
 *
 * @param status 当前内核运行状态
 * @returns 连接状态与操作入口
 */
export function useEngineConnection(
  status: EngineStatusResponse | undefined
): UseEngineConnectionResult {
  const queryClient = useQueryClient();
  const [error, setError] = useState("");

  const invalidate = (): void => {
    void queryClient.invalidateQueries({ queryKey: ["engine-status"] });
  };

  const connect = useMutation({
    mutationFn: api.config.engineConnect,
    onMutate: () => setError(""),
    onSuccess: () => {
      setError("");
      invalidate();
    },
    onError: (cause) => setError(connectionErrorMessage(cause))
  });

  const disconnect = useMutation({
    mutationFn: api.config.engineDisconnect,
    onSuccess: () => {
      setError("");
      invalidate();
    },
    onError: (cause) => setError(connectionErrorMessage(cause))
  });

  return {
    state: resolveEngineConnectionState({
      status,
      connecting: connect.isPending,
      failed: Boolean(error)
    }),
    error,
    connect: () => connect.mutate(),
    disconnect: () => disconnect.mutate()
  };
}
