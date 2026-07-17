import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useEffect, useRef, useState } from "react";
import { createTerminalOptions } from "./terminal-options";
import { connectTerminalSession, type TerminalConnectionStatus } from "./terminal-session-controller";
import "./terminal-pane.css";

/**
 * 渲染与指定终端会话相连的 xterm 实例。
 *
 * @param props 终端会话标识
 * @returns xterm 终端界面
 */
export function TerminalPane({ terminalId }: { terminalId: string }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<TerminalConnectionStatus>("connecting");
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const terminal = new Terminal(createTerminalOptions());
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(container);
    fit.fit();
    const controller = connectTerminalSession({ terminalId, terminal, onStatusChange: setStatus, onError: setError });
    const observer = new ResizeObserver(() => {
      fit.fit();
      controller.resize(terminal.cols, terminal.rows);
    });
    observer.observe(container);
    return () => {
      observer.disconnect();
      controller.dispose();
      terminal.dispose();
    };
  }, [terminalId]);
  return <section className="terminal-pane"><div className="terminal-surface" ref={containerRef} />{status !== "connected" && <span className={`terminal-connection-status ${status}`}>{connectionLabel(status)}</span>}{error && <div className="pane-error terminal-error">{error}</div>}</section>;
}

/**
 * 返回终端连接状态文本。
 *
 * @param status 连接状态
 * @returns 中文状态文本
 */
function connectionLabel(status: TerminalConnectionStatus): string {
  return {
    connecting: "正在连接",
    connected: "已连接",
    reconnecting: "正在重连",
    failed: "连接失败"
  }[status];
}
