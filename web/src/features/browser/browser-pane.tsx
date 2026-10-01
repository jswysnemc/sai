import { useEffect, useState } from "react";
import { Bot, CircleAlert, RefreshCw, X } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { BrowserTabStrip } from "./browser-tab-strip";
import { BrowserToolbar } from "./browser-toolbar";
import { BrowserViewport } from "./browser-viewport";
import { useBrowserSession, type BrowserActivity } from "./use-browser-session";
import "./browser-pane.css";

/** Agent 操作提示在最后一步后保留的时长。 */
const ACTIVITY_VISIBLE_MS = 4_000;
/** 视口尺寸未知时的默认值，与服务端默认视口一致。 */
const DEFAULT_SIZE = { width: 1280, height: 800 };

/**
 * 工作区内置浏览器面板。
 *
 * 面板与 Agent 浏览器工具共用同一个浏览器会话：Agent 操作时这里实时显示画面与步骤，
 * 用户也可以随时在画面上点击、输入接手操作。
 *
 * @returns 浏览器面板
 */
export function BrowserPane() {
  const { t } = useI18n();
  const session = useBrowserSession();
  const agentActivity = useRecentActivity(session.activity);
  const disabled = session.status !== "connected";
  const width = session.state?.width || DEFAULT_SIZE.width;
  const height = session.state?.height || DEFAULT_SIZE.height;

  if (session.status === "failed") {
    return (
      <section className="browser-pane browser-pane-failed" aria-live="polite">
        <CircleAlert size={20} aria-hidden />
        <strong>{t("Browser unavailable", "浏览器不可用")}</strong>
        <p>{session.error ?? t("Connection to the built-in browser was lost.", "与内置浏览器的连接已断开。")}</p>
        <p className="browser-pane-hint">
          {t(
            "Install Chrome, Chromium or Edge, or set SAI_BROWSER_EXECUTABLE before starting sai.",
            "请安装 Chrome、Chromium 或 Edge，或在启动 sai 前设置 SAI_BROWSER_EXECUTABLE。"
          )}
        </p>
        <Button onClick={session.retry}>
          <RefreshCw size={14} aria-hidden />
          <span>{t("Retry", "重试")}</span>
        </Button>
      </section>
    );
  }

  return (
    <section className="browser-pane" aria-busy={session.status !== "connected"}>
      <BrowserTabStrip tabs={session.state?.tabs ?? []} disabled={disabled} onSend={session.send} />
      <BrowserToolbar state={session.state} disabled={disabled} onSend={session.send} />
      <div className="browser-stage">
        {session.state?.loading && <div className="browser-progress" role="progressbar" aria-label={t("Loading page", "页面加载中")} />}
        <BrowserViewport
          pageWidth={width}
          pageHeight={height}
          disabled={disabled}
          onSend={session.send}
          onFrameHandler={session.setFrameHandler}
        />
        <div className="browser-overlay" aria-live="polite">
          {session.status !== "connected" && (
            <span className="browser-chip">
              {session.status === "connecting" ? t("Starting browser…", "正在启动浏览器…") : t("Reconnecting…", "正在重新连接…")}
            </span>
          )}
          {agentActivity && (
            <span className="browser-chip browser-chip-agent">
              <Bot size={12} aria-hidden />
              <span>{t("Agent", "Agent")}: {agentActivity}</span>
            </span>
          )}
          {session.error && (
            <span className="browser-chip browser-chip-error" role="alert">
              <span>{session.error}</span>
              <button type="button" aria-label={t("Dismiss", "关闭提示")} onClick={session.dismissError}>
                <X size={12} aria-hidden />
              </button>
            </span>
          )}
        </div>
      </div>
    </section>
  );
}

/**
 * 只在最近一次 Agent 操作后的数秒内返回提示文案。
 *
 * @param activity 最近一次 Agent 操作
 * @returns 仍在展示期内的操作文案
 */
function useRecentActivity(activity: BrowserActivity | null): string | null {
  const [visible, setVisible] = useState<string | null>(null);
  useEffect(() => {
    if (!activity) return;
    setVisible(activity.message);
    const timer = window.setTimeout(() => setVisible(null), ACTIVITY_VISIBLE_MS);
    return () => window.clearTimeout(timer);
  }, [activity]);
  return visible;
}
