import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useEffect, useRef, useState } from "react";
import { createTerminalOptions } from "./terminal-options";
import { connectTerminalSession, type TerminalConnectionStatus } from "./terminal-session-controller";
import "./terminal-pane.css";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染与指定终端会话相连的 xterm 实例。
 *
 * @param props 终端会话标识
 * @returns xterm 终端界面
 */
export function TerminalPane({ terminalId }: { terminalId: string }) {
  const { t } = useI18n();
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
    const controller = connectTerminalSession({
      terminalId,
      terminal,
      onStatusChange: setStatus,
      onError: setError,
      disconnectedMessage: t("The terminal connection was lost. Select the terminal again or create a new session.", "终端连接已断开，请重新选择终端或新建会话")
    });
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
  }, [t, terminalId]);
  return <section className="terminal-pane"><div className="terminal-surface" ref={containerRef} />{status !== "connected" && <span className={`terminal-connection-status ${status}`}>{connectionLabel(status, t)}</span>}{error && <div className="pane-error terminal-error">{error}</div>}</section>;
}

/**
 * 返回终端连接状态文本。
 *
 * @param status 连接状态
 * @returns 中文状态文本
 */
function connectionLabel(status: TerminalConnectionStatus, t: (en: string, zh: string) => string): string {
  return {
    connecting: t("Connecting", "正在连接"),
    connected: t("Connected", "已连接"),
    reconnecting: t("Reconnecting", "正在重连"),
    failed: t("Connection failed", "连接失败")
  }[status];
}
