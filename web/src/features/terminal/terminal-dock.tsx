import { TerminalSquare } from "lucide-react";
import { TerminalPane } from "./terminal-pane";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染单个终端会话画面，多会话由工作区顶栏标签管理。
 *
 * @param props 当前终端会话 ID 与错误信息
 * @returns 终端视图
 */
export function TerminalDock({
  terminalId,
  title,
  error
}: {
  terminalId?: string | null;
  title: string;
  error?: Error | null;
}) {
  const { t } = useI18n();
  return (
    <section className="terminal-dock terminal-dock-flat">
      <div className="terminal-main">
        {terminalId ? (
          <TerminalPane terminalId={terminalId} title={title} />
        ) : (
          <div className="terminal-empty">
            <TerminalSquare size={22} />
            <p>{t("No active terminal", "没有活动终端")}</p>
          </div>
        )}
        {error && <div className="pane-error terminal-error">{error.message}</div>}
      </div>
    </section>
  );
}
