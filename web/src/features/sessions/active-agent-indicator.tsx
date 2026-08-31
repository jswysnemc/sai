import { useI18n } from "../i18n/use-i18n";

type SessionLoadedIndicatorProps = {
  /** 存活持有者类型；未知时只提示会话已打开 */
  holder?: string | null;
};

/**
 * 渲染终端或网页已加载会话的克制脉冲状态。
 *
 * @param props 可选持有者类型
 * @returns 可访问的加载状态指示器
 */
export function ActiveAgentIndicator({ holder }: SessionLoadedIndicatorProps) {
  const { t } = useI18n();
  return (
    <span className="active-agent-indicator" role="status" aria-label={holderLabel(holder, t)}>
      <span />
    </span>
  );
}

/**
 * 按持有者类型生成加载态说明。
 *
 * @param holder 持有者类型
 * @param t 双语文案选择
 * @returns 无障碍标签
 */
function holderLabel(holder: string | null | undefined, t: (en: string, zh: string) => string): string {
  if (holder === "repl") return t("Open in the terminal", "终端已打开");
  if (holder === "web") return t("Open in the web app", "网页已打开");
  if (holder === "gateway") return t("Open in a gateway", "网关已打开");
  return t("Session is open", "会话已打开");
}
