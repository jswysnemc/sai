import { TerminalSquare } from "lucide-react";
import { TerminalPane } from "./terminal-pane";

/**
 * 渲染单个终端会话画面，多会话由工作区顶栏标签管理。
 *
 * @param props 当前终端会话 ID 与错误信息
 * @returns 终端视图
 */
export function TerminalDock({
  terminalId,
  error
}: {
  terminalId?: string | null;
  error?: Error | null;
}) {
  return (
    <section className="terminal-dock terminal-dock-flat">
      <div className="terminal-main">
        {terminalId ? (
          <TerminalPane terminalId={terminalId} />
        ) : (
          <div className="terminal-empty">
            <TerminalSquare size={22} />
            <p>没有活动终端</p>
          </div>
        )}
        {error && <div className="pane-error terminal-error">{error.message}</div>}
      </div>
    </section>
  );
}
