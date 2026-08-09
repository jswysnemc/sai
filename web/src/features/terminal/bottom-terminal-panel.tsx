import { SquareTerminal, X } from "lucide-react";
import { useEffect, useRef, useState, type CSSProperties } from "react";
import type { TerminalManager } from "./use-terminal-manager";
import { TerminalPane } from "./terminal-pane";
import { useI18n } from "../i18n/use-i18n";
import { Button } from "../../shared/ui/button/button";
import { SshKnownHostPrompt } from "../settings/ssh/ssh-known-host-prompt";
import { SshTargetPicker } from "./ssh-target-picker";
import "./bottom-terminal-panel.css";

type BottomTerminalPanelProps = {
  manager: TerminalManager;
  height: number;
  onResize: (height: number) => void;
  onClose: () => void;
};

/**
 * 渲染工作台底部终端，提供多会话标签、创建、关闭和高度调整。
 *
 * @param props 终端管理器、面板高度与布局回调
 * @returns 底部终端面板
 */
export function BottomTerminalPanel({ manager, height, onResize, onClose }: BottomTerminalPanelProps) {
  const { t } = useI18n();
  const [resizing, setResizing] = useState(false);
  const initialCreateAttempted = useRef(false);
  const active = manager.terminals.find((terminal) => terminal.id === manager.activeId) ?? manager.terminals[0];

  useEffect(() => {
    if (manager.terminals.length > 0 || initialCreateAttempted.current) return;
    initialCreateAttempted.current = true;
    void manager.createTerminal().catch(() => undefined);
  }, [manager, manager.terminals.length]);

  useEffect(() => {
    if (!resizing) return;
    const handleMove = (event: PointerEvent) => {
      onResize(window.innerHeight - event.clientY);
    };
    const handleUp = () => setResizing(false);
    window.addEventListener("pointermove", handleMove);
    window.addEventListener("pointerup", handleUp, { once: true });
    return () => {
      window.removeEventListener("pointermove", handleMove);
      window.removeEventListener("pointerup", handleUp);
    };
  }, [onResize, resizing]);

  /**
   * 创建一个终端并切换到新标签。
   *
   * @returns 无返回值
   */
  const createTerminal = () => {
    void manager.createTerminal().catch(() => undefined);
  };

  return (
    <section
      className={`bottom-terminal-panel${resizing ? " is-resizing" : ""}`}
      style={{ "--terminal-panel-height": `${height}px` } as CSSProperties}
      aria-label={t("Bottom terminal", "底部终端")}
    >
      <Button
        className="bottom-terminal-resize-handle"
        onPointerDown={() => setResizing(true)}
        aria-label={t("Resize terminal", "调整终端高度")}
        title={t("Resize terminal", "调整终端高度")}
      ><span /></Button>
      <header className="bottom-terminal-head">
        <div className="bottom-terminal-tabs" role="tablist" aria-label={t("Terminal sessions", "终端会话") }>
          {manager.terminals.map((terminal) => (
            <div className={`bottom-terminal-tab${terminal.id === active?.id ? " active" : ""}`} key={terminal.id}>
              <Button role="tab" aria-selected={terminal.id === active?.id} onClick={() => manager.setActiveId(terminal.id)}>
                <SquareTerminal size={12} />
                <span>{terminal.title || t("Terminal", "终端")}</span>
              </Button>
              <Button
                className="bottom-terminal-tab-close"
                onClick={() => void manager.closeTerminal(terminal.id)}
                aria-label={t(`Close ${terminal.title}`, `关闭 ${terminal.title}`)}
                title={t("Close terminal", "关闭终端")}
              >
                <X size={11} />
              </Button>
            </div>
          ))}
          <SshTargetPicker
            onCreateLocal={createTerminal}
            onCreateSsh={(hostId) => void manager.createSshTerminal(hostId).catch(() => undefined)}
          />
        </div>
        <div className="bottom-terminal-actions">
          <Button onClick={onClose} aria-label={t("Hide terminal panel", "隐藏终端面板")} title={t("Hide terminal panel", "隐藏终端面板")}>
            <X size={14} />
          </Button>
        </div>
      </header>
      <div className="bottom-terminal-body">
        {active ? <TerminalPane terminalId={active.id} title={active.title} /> : <div className="bottom-terminal-empty">{t("Create a terminal to begin", "新建终端后开始使用")}</div>}
      </div>
      <SshKnownHostPrompt
        prompt={manager.hostKeyPrompt}
        busy={false}
        onTrust={() => void manager.trustHostKeyAndRetry().catch(() => undefined)}
        onCancel={manager.dismissHostKeyPrompt}
      />
    </section>
  );
}
