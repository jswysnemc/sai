import { Brain, ChevronRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNestedFollowOutputScroll } from "./use-follow-output-scroll";
import "./reasoning-block.css";
import "./message/activity-stream.css";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染可折叠的思考过程及耗时。
 *
 * @param props 思考文本、实时状态和起止时间
 * @returns sai-chat 风格思考区域
 */
export function ReasoningBlock({ source, live, startedAt, endedAt }: { source: string; live?: boolean; startedAt?: string; endedAt?: string }) {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(Boolean(live));
  // 用户手动开合后不再跟随流式状态，否则一轮结束会把展开的内容强制收起
  const [userToggled, setUserToggled] = useState(false);
  const [clock, setClock] = useState(() => Date.now());
  const contentRef = useRef<HTMLDivElement>(null);

  // 1. 流式输出时自动展开，结束或历史加载时自动收起；用户已表态则保持其选择
  useEffect(() => {
    if (userToggled) return;
    setOpen(Boolean(live));
  }, [live, userToggled]);

  // 2. 流式期间每秒刷新耗时显示
  useEffect(() => {
    if (!live || !startedAt) return;
    const timer = window.setInterval(() => setClock(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, [live, startedAt]);

  // 3. 长思考块跟随底部；用户上滚查看时不抢焦点，回到底部后恢复
  useNestedFollowOutputScroll(contentRef, source, Boolean(live && open));

  if (!source) return null;
  const duration = reasoningDuration(startedAt, endedAt, clock, locale);
  return (
    <section className={`reasoning-block${open ? " open" : ""}`}>
      <button
        type="button"
        onClick={() => {
          setUserToggled(true);
          setOpen((value) => !value);
        }}
      >
        <span className="reasoning-icon" aria-hidden><Brain size={14} /></span>
        <span className="reasoning-label">{live ? t("Thinking", "正在思考") : t("Reasoning", "思考过程")}{duration ? t(` (${duration})`, `（用时 ${duration}）`) : ""}</span>
        <ChevronRight size={14} />
      </button>
      {open && <div ref={contentRef} className="reasoning-content" tabIndex={-1}>{source}</div>}
    </section>
  );
}

/**
 * 根据可用时间计算思考耗时。
 *
 * @param startedAt 开始时间
 * @param endedAt 结束时间
 * @param clock 实时计时参考值
 * @returns 人类可读耗时，时间无效时返回空文本
 */
export function reasoningDuration(startedAt: string | undefined, endedAt: string | undefined, clock: number, locale: "en-US" | "zh-CN"): string {
  if (!startedAt) return "";
  const start = Date.parse(startedAt);
  const end = endedAt ? Date.parse(endedAt) : clock;
  if (!Number.isFinite(start) || !Number.isFinite(end) || end < start) return "";
  const seconds = Math.max(1, Math.round((end - start) / 1_000));
  if (seconds < 60) return locale === "zh-CN" ? `${seconds} 秒` : `${seconds}s`;
  return locale === "zh-CN"
    ? `${Math.floor(seconds / 60)} 分 ${seconds % 60} 秒`
    : `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}
