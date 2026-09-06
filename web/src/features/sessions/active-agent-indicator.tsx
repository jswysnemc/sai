import { useI18n } from "../i18n/use-i18n";

type SessionRunningIndicatorProps = {
  /** 运行持有者类型；未知时只提示会话正在工作 */
  holder?: string | null;
};

/**
 * 渲染会话实际正在工作的状态。
 *
 * @param props 可选持有者类型
 * @returns 可访问的运行状态指示器
 */
export function ActiveAgentIndicator({ holder }: SessionRunningIndicatorProps) {
  const { t } = useI18n();
  return (
    <span className="active-agent-indicator" role="status" aria-label={holderLabel(holder, t)}>
      <span />
    </span>
  );
}

/**
 * 按持有者类型生成运行说明。
 *
 * @param holder 持有者类型
 * @param t 双语文案选择
 * @returns 无障碍标签
 */
function holderLabel(holder: string | null | undefined, t: (en: string, zh: string) => string): string {
  if (holder === "repl") return t("Working in the terminal", "终端会话正在工作");
  if (holder === "web") return t("Working in the web app", "网页会话正在工作");
  if (holder === "gateway") return t("Working in a gateway", "网关会话正在工作");
  return t("Session is working", "会话正在工作");
}
