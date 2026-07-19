import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useEffect, useRef, useState } from "react";
import { FOCUS_COMPOSER_EVENT, INSERT_TERMINAL_SELECTION_EVENT, type TerminalSelectionDetail } from "../chat/composer/composer-events";
import { createTerminalOptions } from "./terminal-options";
import { connectTerminalSession, type TerminalConnectionStatus } from "./terminal-session-controller";
import { TerminalContextMenu } from "./terminal-context-menu";
import "./terminal-pane.css";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染与指定终端会话相连的 xterm 实例。
 *
 * @param props 终端会话标识
 * @returns xterm 终端界面
 */
export function TerminalPane({ terminalId, title }: { terminalId: string; title: string }) {
  const { t } = useI18n();
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<TerminalConnectionStatus>("connecting");
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; selection: string } | null>(null);
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const terminal = new Terminal(createTerminalOptions());
    terminalRef.current = terminal;
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
    /** 使用终端选区打开专用右键菜单。 */
    const handleContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      setContextMenu({ x: event.clientX, y: event.clientY, selection: terminal.getSelection() });
    };
    container.addEventListener("contextmenu", handleContextMenu);
    return () => {
      container.removeEventListener("contextmenu", handleContextMenu);
      observer.disconnect();
      controller.dispose();
      terminal.dispose();
      terminalRef.current = null;
    };
  }, [t, terminalId]);

  /** 将终端剪贴板错误显示在当前面板。 */
  const handleClipboardError = (reason: unknown) => {
    setError(reason instanceof Error ? reason.message : t("Clipboard access failed", "剪贴板访问失败"));
    setContextMenu(null);
  };

  /** 复制当前终端选区。 */
  const copySelection = () => {
    if (!contextMenu?.selection) return;
    void navigator.clipboard.writeText(contextMenu.selection).then(() => setContextMenu(null), handleClipboardError);
  };

  /** 从系统剪贴板粘贴文本到终端。 */
  const pasteClipboard = () => {
    void navigator.clipboard.readText().then((value) => {
      terminalRef.current?.paste(value);
      setContextMenu(null);
    }, handleClipboardError);
  };

  /** 将当前终端选区作为输入原子发送到聊天输入框。 */
  const sendSelectionToChat = () => {
    if (!contextMenu?.selection) return;
    window.dispatchEvent(new CustomEvent<TerminalSelectionDetail>(INSERT_TERMINAL_SELECTION_EVENT, {
      detail: { source: title, content: contextMenu.selection }
    }));
    terminalRef.current?.clearSelection();
    setContextMenu(null);
    requestAnimationFrame(() => window.dispatchEvent(new Event(FOCUS_COMPOSER_EVENT)));
  };

  return <section className="terminal-pane">
    <div className="terminal-surface" ref={containerRef} />
    {status !== "connected" && <span className={`terminal-connection-status ${status}`}>{connectionLabel(status, t)}</span>}
    {error && <div className="pane-error terminal-error">{error}</div>}
    {contextMenu && (
      <TerminalContextMenu
        x={contextMenu.x}
        y={contextMenu.y}
        hasSelection={Boolean(contextMenu.selection)}
        onCopy={copySelection}
        onPaste={pasteClipboard}
        onSelectAll={() => {
          terminalRef.current?.selectAll();
          setContextMenu(null);
        }}
        onSendToChat={sendSelectionToChat}
        onClose={() => setContextMenu(null)}
      />
    )}
  </section>;
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
