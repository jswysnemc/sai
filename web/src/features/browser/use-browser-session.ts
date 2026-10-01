import { useCallback, useEffect, useRef, useState } from "react";
import {
  browserSocketUrl,
  parseBrowserServerMessage,
  type BrowserClientMessage,
  type BrowserState
} from "./browser-protocol";

/** 连接状态。 */
export type BrowserConnectionStatus = "connecting" | "connected" | "reconnecting" | "failed";

/** Agent 操作提示，带时间戳以便同文案重复出现时也能刷新。 */
export type BrowserActivity = { message: string; at: number };

/** 画面回调：每收到一帧 JPEG 调用一次。 */
export type BrowserFrameHandler = (frame: Blob) => void;

/** 自动重连的最大次数，超过后转为失败并等待用户重试。 */
const MAX_RECONNECT_ATTEMPTS = 5;

/**
 * 管理内置浏览器面板的 WebSocket 会话。
 *
 * 画面帧不进入 React 状态，直接交给视口注册的回调绘制，避免每帧触发重渲染。
 *
 * @returns 连接状态、浏览器状态、发送方法与画面回调注册方法
 */
export function useBrowserSession() {
  const [status, setStatus] = useState<BrowserConnectionStatus>("connecting");
  const [state, setState] = useState<BrowserState | null>(null);
  const [activity, setActivity] = useState<BrowserActivity | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [generation, setGeneration] = useState(0);
  const socketRef = useRef<WebSocket | null>(null);
  const frameHandlerRef = useRef<BrowserFrameHandler | null>(null);

  useEffect(() => {
    let disposed = false;
    let attempts = 0;
    let everConnected = false;
    let timer: number | undefined;
    let lastServerError: string | null = null;

    /** 建立一次连接；断开后按退避策略重连。 */
    const connect = () => {
      if (disposed) return;
      setStatus(attempts === 0 ? "connecting" : "reconnecting");
      const socket = new WebSocket(browserSocketUrl());
      socket.binaryType = "blob";
      socketRef.current = socket;
      socket.onmessage = (event) => {
        if (disposed || socketRef.current !== socket) return;
        // 1. 二进制帧是页面画面
        if (event.data instanceof Blob) {
          frameHandlerRef.current?.(event.data);
          return;
        }
        // 2. 文本帧是状态、Agent 操作或错误
        const message = parseBrowserServerMessage(String(event.data));
        if (!message) return;
        if (message.type === "state") {
          everConnected = true;
          attempts = 0;
          setStatus("connected");
          setError(null);
          setState(message.state);
        } else if (message.type === "activity") {
          setActivity({ message: message.message, at: Date.now() });
        } else {
          lastServerError = message.message;
          setError(message.message);
        }
      };
      socket.onerror = () => socket.close();
      socket.onclose = () => {
        if (socketRef.current === socket) socketRef.current = null;
        if (disposed) return;
        // 3. 从未连通（如本机没有浏览器）或重连次数用尽时停止自动重试
        attempts += 1;
        if (!everConnected || attempts > MAX_RECONNECT_ATTEMPTS) {
          setStatus("failed");
          if (lastServerError) setError(lastServerError);
          return;
        }
        setStatus("reconnecting");
        timer = window.setTimeout(connect, Math.min(3_000, 300 * 2 ** (attempts - 1)));
      };
    };

    connect();
    return () => {
      disposed = true;
      window.clearTimeout(timer);
      socketRef.current?.close();
      socketRef.current = null;
    };
  }, [generation]);

  /** 发送控制消息；连接未就绪时丢弃。 */
  const send = useCallback((message: BrowserClientMessage) => {
    const socket = socketRef.current;
    if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
  }, []);

  /** 注册画面回调，传 null 注销。 */
  const setFrameHandler = useCallback((handler: BrowserFrameHandler | null) => {
    frameHandlerRef.current = handler;
  }, []);

  /** 失败后手动重连。 */
  const retry = useCallback(() => {
    setError(null);
    setGeneration((value) => value + 1);
  }, []);

  /** 关闭错误提示。 */
  const dismissError = useCallback(() => setError(null), []);

  return { status, state, activity, error, send, setFrameHandler, retry, dismissError };
}
