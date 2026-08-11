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
  const panelRef = useRef<HTMLElement>(null);
  const active = manager.terminals.find((terminal) => terminal.id === manager.activeId) ?? manager.terminals[0];

  useEffect(() => {
    if (manager.terminals.length > 0 || initialCreateAttempted.current) return;
    initialCreateAttempted.current = true;
    void manager.createTerminal().catch(() => undefined);
  }, [manager, manager.terminals.length]);

  // #region agent log
  useEffect(() => {
    const panel = panelRef.current;
    if (!panel) return;
    const handle = panel.querySelector(".bottom-terminal-resize-handle") as HTMLElement | null;
    const plus = panel.querySelector(".bottom-terminal-new") as HTMLElement | null;
    const reportGeometry = (reason: string) => {
      const handleRect = handle?.getBoundingClientRect();
      const plusRect = plus?.getBoundingClientRect();
      const handleStyle = handle ? getComputedStyle(handle) : null;
      fetch('http://127.0.0.1:7716/ingest/0150b615-e4f4-4cb9-b2bc-b348cdf7556f',{method:'POST',headers:{'Content-Type':'application/json','X-Debug-Session-Id':'dcb5f5'},body:JSON.stringify({sessionId:'dcb5f5',runId:'pre-fix',hypothesisId:'A',location:'bottom-terminal-panel.tsx:geometry',message:reason,data:{handle:{top:handleRect?.top,left:handleRect?.left,width:handleRect?.width,height:handleRect?.height,zIndex:handleStyle?.zIndex,minHeight:handleStyle?.minHeight,heightCss:handleStyle?.height,pointerEvents:handleStyle?.pointerEvents},plus:{top:plusRect?.top,left:plusRect?.left,width:plusRect?.width,height:plusRect?.height},overlaps:Boolean(handleRect&&plusRect&&handleRect.bottom>plusRect.top&&handleRect.top<plusRect.bottom&&handleRect.left<plusRect.right&&handleRect.right>plusRect.left)},timestamp:Date.now()})}).catch(()=>{});
    };
    reportGeometry("terminal panel geometry on mount");
    const onPointerDownCapture = (event: PointerEvent) => {
      const under = document.elementFromPoint(event.clientX, event.clientY);
      const path = event.composedPath().slice(0, 6).map((node) => {
        if (node instanceof Element) {
          return `${node.nodeName}.${String(node.className || "").toString().split(" ").slice(0, 3).join(".")}`;
        }
        if (node instanceof Node) return node.nodeName;
        return String(node);
      });
      fetch('http://127.0.0.1:7716/ingest/0150b615-e4f4-4cb9-b2bc-b348cdf7556f',{method:'POST',headers:{'Content-Type':'application/json','X-Debug-Session-Id':'dcb5f5'},body:JSON.stringify({sessionId:'dcb5f5',runId:'pre-fix',hypothesisId:'B',location:'bottom-terminal-panel.tsx:pointerdown-capture',message:'pointerdown inside terminal panel',data:{clientX:event.clientX,clientY:event.clientY,underTag:under?.nodeName,underClass:(under as Element|null)?.className??null,path,targetClass:(event.target as Element|null)?.className??null},timestamp:Date.now()})}).catch(()=>{});
    };
    panel.addEventListener("pointerdown", onPointerDownCapture, true);
    return () => panel.removeEventListener("pointerdown", onPointerDownCapture, true);
  }, []);
  // #endregion

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
      ref={panelRef}
      className={`bottom-terminal-panel${resizing ? " is-resizing" : ""}`}
      style={{ "--terminal-panel-height": `${height}px` } as CSSProperties}
      aria-label={t("Bottom terminal", "底部终端")}
    >
      <Button
        className="bottom-terminal-resize-handle"
        onPointerDown={() => {
          // #region agent log
          fetch('http://127.0.0.1:7716/ingest/0150b615-e4f4-4cb9-b2bc-b348cdf7556f',{method:'POST',headers:{'Content-Type':'application/json','X-Debug-Session-Id':'dcb5f5'},body:JSON.stringify({sessionId:'dcb5f5',runId:'pre-fix',hypothesisId:'A',location:'bottom-terminal-panel.tsx:resize-handle',message:'resize handle pointerdown fired',data:{},timestamp:Date.now()})}).catch(()=>{});
          // #endregion
          setResizing(true);
        }}
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
